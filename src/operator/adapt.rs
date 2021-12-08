/*! Declarative approach to adapting input data in one format to output data in another.

Example:

```js
adapt from: neut_deg  to: enut_rad
```

We introduce the coordinate type designations *eastish, northish, upish, timish*,
and their geometrical inverses *westish, southish, downish, reversed-timeish*,
with mostly evident meaning: A coordinate is *eastish* if you would typically draw
it along an abscissa, *northish* if you would typically draw it along an ordinate,
*upish* if you would need to draw it out of the paper, and "timeish"
if it represents ordinary, forward evolving time. *Westish, southish, downish*, and
*reversed-timeish* are the axis-reverted versions of the former four.

These 8 spatio-temporal directional designations have convenient short forms,
`e, n, u, t` and `w, s, d, r`, respectively.

Also, we introduce the 3 common angular representations "degrees, gradians, radians",
conveniently abbrevieated as "deg", "gon" and "rad".

The Rust Geodesy internal format of a four dimensional coordinate tuple is e, n, u, t,
and the internal unit of measure for anglular coordinates is radians. In `adapt`, terms,
this is described as `enut_rad`.

`adapt` covers the same ground as the `PROJ` operator `axisswap`, but using a somewhat
different approach: You never tell `adapt` what you want it to do - you only tell it
what you want to go `from`, and what you want to come `to` (and in most cases actually
only one of those). Then `adapt` figures out how to fulfill the order.

The example above specifies that an input coordinate tuple with coordinate order
**latitude, longitude, height, time**, with latitude and longitude in degrees, should be
converted to an output coordinate in radians and with latitude and longitude swapped.
That output format is identical to the default internal format, so it can actually
be left out, and the order be written as:

```gys
adapt from: neut_deg
```

Typically, `adapt` is used in both ends of a pipeline, to match data between the
RG internal representation and the requirements of the embedding system:

```gys
adapt from: neut_deg | cart ... | helmert ... | cart inv ... | adapt to: neut_deg
```

Note that `adapt to: ...` and `adapt inv from: ...` are equivalent. The latter
form is useful when using RG's predefined symbolic definitions, as in:

```gys
geo | cart ... | helmert ... | cart inv ... | geo inv
```

!*/

use crate::CoordinateTuple;
use crate::GeodesyError;
use crate::GysResource;
use crate::Operator;
use crate::OperatorCore;
use crate::Provider;

#[derive(Debug, Default, Clone)]
pub struct Adapt {
    args: Vec<(String, String)>,
    inverted: bool,
    post: [usize; 4],
    mult: [f64; 4],
    noop: bool,
}

#[derive(Debug, Default, Clone)]
struct CoordinateOrderDescriptor {
    post: [usize; 4],
    mult: [f64; 4],
    noop: bool,
}

#[allow(clippy::float_cmp)]
fn descriptor(desc: &str) -> Option<CoordinateOrderDescriptor> {
    let mut post = [0_usize, 1, 2, 3];
    let mut mult = [1_f64, 1., 1., 1.];
    if desc == "pass" {
        return Some(CoordinateOrderDescriptor {
            post,
            mult,
            noop: true,
        });
    }

    if desc.len() != 4 && desc.len() != 8 {
        return None;
    }

    let mut torad = 1_f64;
    if desc.len() == 8 {
        let good_angular = desc.ends_with("_deg")
            || desc.ends_with("_gon")
            || desc.ends_with("_rad")
            || desc.ends_with("_any");
        if !good_angular {
            return None;
        }
        if desc.ends_with("_deg") {
            torad = std::f64::consts::PI / 180.;
        } else if desc.ends_with("_gon") {
            torad = std::f64::consts::PI / 200.;
        }
    }

    // Now figure out what goes (resp. comes from) where
    let desc: Vec<char> = desc[0..4].chars().collect();
    let mut indices = [1i32, 2, 3, 4];
    for i in 0..4 {
        let d = desc[i];

        // Unknown designator
        if !"neutswdr".contains(d) {
            return None;
        }
        // Sign and position in the internal representation
        let dd: i32 = match d {
            'w' => -1,
            's' => -2,
            'd' => -3,
            'r' => -4,
            'e' => 1,
            'n' => 2,
            'u' => 3,
            't' => 4,
            _ => 0, // cannot happen: We already err'ed on unknowns
        };
        indices[i] = dd;
    }

    // Check that the descriptor describes a true permutation:
    // all inputs go to a unique output
    let mut count = [0_usize, 0, 0, 0];
    for i in 0..4 {
        count[(indices[i].abs() - 1) as usize] += 1;
    }
    if count != [1, 1, 1, 1] {
        return None;
    }

    // Now untangle the sign and position parts of 'indices'
    for i in 0..4 {
        let d = indices[i];
        post[i] = (d.abs() - 1) as usize;
        mult[i] = d.signum() as f64 * if i > 1 { 1.0 } else { torad };
    }
    let noop = mult == [1.0; 4] && post == [0_usize, 1, 2, 3];

    Some(CoordinateOrderDescriptor { post, mult, noop })
}

#[allow(clippy::float_cmp)]
fn combine_descriptors(
    from: &CoordinateOrderDescriptor,
    to: &CoordinateOrderDescriptor,
) -> CoordinateOrderDescriptor {
    let mut give = CoordinateOrderDescriptor::default();
    for i in 0..4 {
        give.mult[i] = from.mult[i] / to.mult[i];
        give.post[i] = from.post.iter().position(|&p| p == to.post[i]).unwrap();
    }
    give.noop = give.mult == [1.0; 4] && give.post == [0_usize, 1, 2, 3];
    give
}

impl Adapt {
    pub fn new(res: &GysResource) -> Result<Adapt, GeodesyError> {
        let mut args = res.to_args(0)?;
        let inverted = args.flag("inv");

        // What we go `from` and what we go `to` both defaults to the internal
        // representation - i.e. "do nothing", neither on in- or output.
        let mut from = args.string("from", "enut");
        let mut to = args.string("to", "enut");

        // forward and inverse give very slightly different results, due to the
        // roundoff difference betweeen multiplication and division. We avoid
        // that by swapping the "from" and "to" descriptors instead, and handling
        // the unconventional calling logic by overwriting the default `operate`
        // method below.
        if inverted {
            std::mem::swap(&mut to, &mut from);
        }

        let desc = descriptor(&from);
        if desc.is_none() {
            return Err(GeodesyError::Operator("Adapt", "Bad value for 'from'"));
        }
        let from = desc.unwrap();

        let desc = descriptor(&to);
        if desc.is_none() {
            return Err(GeodesyError::Operator("Adapt", "Bad value for 'to'"));
        }
        let to = desc.unwrap();

        // Eliminate redundancy for over-specified cases.
        let give = combine_descriptors(&from, &to);

        Ok(Adapt {
            args: args.used,
            inverted,
            post: give.post,
            mult: give.mult,
            noop: give.noop,
        })
    }

    pub(crate) fn operator(
        args: &GysResource,
        _rp: &dyn Provider,
    ) -> Result<Operator, GeodesyError> {
        let op = crate::operator::adapt::Adapt::new(args)?;
        Ok(Operator(Box::new(op)))
    }
}

impl OperatorCore for Adapt {
    fn fwd(&self, _ctx: &dyn Provider, operands: &mut [CoordinateTuple]) -> bool {
        if self.noop {
            return true;
        }
        for o in operands {
            *o = CoordinateTuple([
                o[self.post[0]] * self.mult[0],
                o[self.post[1]] * self.mult[1],
                o[self.post[2]] * self.mult[2],
                o[self.post[3]] * self.mult[3],
            ]);
        }
        true
    }

    fn inv(&self, _ctx: &dyn Provider, operands: &mut [CoordinateTuple]) -> bool {
        if self.noop {
            return true;
        }
        for o in operands {
            let mut c = CoordinateTuple::default();
            for i in 0..4_usize {
                c[self.post[i]] = o[i] / self.mult[self.post[i]];
            }
            *o = c;
        }
        true
    }

    // We overwrite the default `operate` in order to handle the trick above,
    // where we swap `from` and `to`, rather than letting `operate` call the
    // complementary method.
    fn operate(&self, ctx: &dyn Provider, operands: &mut [CoordinateTuple], forward: bool) -> bool {
        if forward {
            return self.fwd(ctx, operands);
        }
        self.inv(ctx, operands)
    }

    fn name(&self) -> &'static str {
        "adapt"
    }

    fn debug(&self) -> String {
        format!("{:#?}", self)
    }

    fn is_noop(&self) -> bool {
        self.noop
    }

    fn is_inverted(&self) -> bool {
        self.inverted
    }

    fn args(&self, _step: usize) -> &[(String, String)] {
        &self.args
    }
}

#[cfg(test)]
mod tests {
    use crate::GeodesyError;
    use crate::Provider;
    #[test]
    fn descriptor() {
        use super::combine_descriptors;
        use super::descriptor;

        // Axis swap n<->e
        assert_eq!([1usize, 0, 2, 3], descriptor("neut").unwrap().post);

        // Axis inversion for n+u. Check for all valid angular units
        assert_eq!([1usize, 0, 2, 3], descriptor("sedt_rad").unwrap().post);
        assert_eq!([1usize, 0, 2, 3], descriptor("sedt_gon").unwrap().post);
        assert_eq!([1usize, 0, 2, 3], descriptor("sedt_deg").unwrap().post);
        assert_eq!([-1., 1., -1., 1.], descriptor("sedt_any").unwrap().mult);

        // noop
        assert_eq!(false, descriptor("sedt_any").unwrap().noop);
        assert_eq!(true, descriptor("enut_any").unwrap().noop);
        assert_eq!(true, descriptor("enut_rad").unwrap().noop);
        assert_eq!(true, descriptor("enut").unwrap().noop);
        assert_eq!(true, descriptor("pass").unwrap().noop);

        // Invalid angular unit "pap"
        assert!(descriptor("sedt_pap").is_none());

        // Invalid: Overlapping axes, "ns"
        assert!(descriptor("nsut").is_none());

        // Now a combination, where we swap both axis order and orientation
        let from = descriptor("neut_deg").unwrap();
        let to = descriptor("wndt_gon").unwrap();
        let give = combine_descriptors(&from, &to);
        assert_eq!([1_usize, 0, 2, 3], give.post);
        assert!(give.mult[0] + 400. / 360. < 1e-10); // mult[0] is negative for westish
        assert!(give.mult[1] - 400. / 360. < 1e-10); // mult[1] is positive for northish
        assert!(give.mult[2] + 1.0 < 1e-10); // mult[2] is negative for downish
        assert!(give.mult[3] - 1.0 < 1e-10); // mult[3] is positive for timeish
        assert!(give.noop == false);
    }

    #[test]
    fn adapt() -> Result<(), GeodesyError> {
        use crate::CoordinateTuple;
        let mut ctx = crate::resource::plain::PlainResourceProvider::default();

        let gonify = ctx.define_operation("adapt from:neut_deg   to:enut_gon")?;
        dbg!(gonify);
        let op = ctx.get_operation(gonify)?;
        dbg!(op);
        let mut operands = [
            CoordinateTuple::raw(90., 180., 0., 0.),
            CoordinateTuple::raw(45., 90., 0., 0.),
        ];

        dbg!(operands);
        assert_eq!(ctx.fwd(gonify, &mut operands), true);
        dbg!(operands);
        assert!((operands[0][0] - 200.0).abs() < 1e-10);
        assert!((operands[0][1] - 100.0).abs() < 1e-10);
        assert!((operands[1][0] - 100.0).abs() < 1e-10);
        assert!((operands[1][1] - 50.0).abs() < 1e-10);

        ctx.inv(gonify, &mut operands);
        assert!((operands[0][0] - 90.0).abs() < 1e-10);
        assert!((operands[0][1] - 180.0).abs() < 1e-10);
        assert!((operands[1][0] - 45.0).abs() < 1e-10);
        assert!((operands[1][1] - 90.0).abs() < 1e-10);

        Ok(())
    }
}
