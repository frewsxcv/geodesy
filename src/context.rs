use std::collections::HashMap;

use crate::CoordinateTuple;
use crate::Operator;
use crate::OperatorCore;

#[derive(Debug, Default)]
struct Resource {
    bbox: CoordinateTuple
}

impl Resource {
    #[must_use]
    pub fn _new() -> Resource {
        Resource {
            bbox: CoordinateTuple(0., 0., 0., 0.)
        }
    }
}

#[allow(dead_code)] // unused: resources
#[derive(Default)]
pub struct Context {
    pub coord: CoordinateTuple,
    pub stack: Vec<f64>,
    pub coordinate_stack: Vec<CoordinateTuple>,
    resources: HashMap<String, Resource>,
    pub(crate) user_defined_operators: HashMap<String, NewOperator>,
    pub(crate) last_failing_operation: &'static str,
    pub(crate) cause: &'static str,
}

use crate::OperatorArgs;
use crate::operator::NewOperator;
use crate::operator::operator_factory;

impl Context {
    #[must_use]
    pub fn new() -> Context {
        Context {
            coord: CoordinateTuple(0., 0., 0., 0.),
            stack: vec![],
            coordinate_stack: vec![],
            resources: HashMap::new(),
            last_failing_operation: "",
            cause: "",
            user_defined_operators: HashMap::new(),
        }
    }

    pub fn operate(&mut self, operator: &Operator, forward: bool) -> bool {
        operator.operate(self, forward)
    }

    pub fn register(&mut self, name: String, constructor: NewOperator) {
        self.user_defined_operators.insert(name, constructor);
    }

    pub fn operator(&self, args: &mut OperatorArgs) -> Result<Operator, String> {
        operator_factory(args, Some(self))
    }

    /*
    pub fn operator_factory(&self, args: &mut OperatorArgs) -> Result<Operator, String> {
        use crate::operator as co;

        // Pipelines do not need to be named "pipeline": They are characterized simply
        // by containing steps.
        if args.name == "pipeline" || args.numeric_value("operator_factory", "_nsteps", 0.0)? > 0.0 {
            let op = co::pipeline::Pipeline::new(args)?;
            return Ok(Operator(Box::new(op)));
        }
        if args.name == "cart" {
            let op = co::cart::Cart::new(args)?;
            return Ok(Operator(Box::new(op)));
        }
        if args.name == "helmert" {
            let op = co::helmert::Helmert::new(args)?;
            return Ok(Operator(Box::new(op)));
        }
        if args.name == "tmerc" {
            let op = co::tmerc::Tmerc::new(args)?;
            return Ok(Operator(Box::new(op)));
        }
        if args.name == "utm" {
            let op = co::tmerc::Tmerc::utm(args)?;
            return Ok(Operator(Box::new(op)));
        }
        if args.name == "noop" {
            let op = co::noop::Noop::new(args)?;
            return Ok(Operator(Box::new(op)));
        }

        // Herefter: Søg efter 'name' i filbøtten
        Err(format!("Unknown operator '{}'", args.name))
    }
    */

}

//----------------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    #[test]
    fn operand() {
        use crate::Context;
        let ond = Context::new();
        assert_eq!(ond.stack.len(), 0);
        assert_eq!(ond.coordinate_stack.len(), 0);
        assert_eq!(ond.coord.0, 0.);
        assert_eq!(ond.coord.1, 0.);
        assert_eq!(ond.coord.2, 0.);
        assert_eq!(ond.coord.3, 0.);
    }

    #[test]
    fn operate() {
        use crate::Operator;
        use crate::Context;
        use crate::{fwd, inv};
        let pipeline = "ed50_etrs89: {
            steps: [
                cart: {ellps: intl},
                helmert: {dx: -87, dy: -96, dz: -120},
                cart: {inv: true, ellps: GRS80}
            ]
        }";
        let mut ond = Context::new();
        let op = Operator::new(pipeline, None).unwrap();
        ond.coord = crate::CoordinateTuple::deg(12., 55., 100., 0.);
        ond.operate(&op, fwd);
        assert!((ond.coord.to_degrees().0 - 11.998815342385206861).abs() < 1e-12);
        assert!((ond.coord.to_degrees().1 - 54.999382648950991381).abs() < 1e-12);
        println!("{:?}", ond.coord.to_degrees());
        ond.operate(&op, inv);
        let e = ond.coord.to_degrees();
        println!("{:?}", e);
        assert!((ond.coord.to_degrees().0 - 12.).abs() < 1e-12);
        assert!((ond.coord.to_degrees().1 - 55.).abs() < 1e-12);
    }

}
