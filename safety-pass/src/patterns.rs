/*!

  Simple cell patterns.

*/

use crate::{Cell, CellType, Create, Pattern, Primitive, Replace};
use log::debug;
use safety_net::{DrivenNet, Error, Instantiable, NetRef};
use std::fmt;

/// A * A = A
#[derive(Debug)]
pub struct Idempotent;

impl fmt::Display for Idempotent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "A * A = A")
    }
}

impl Pattern for Idempotent {
    type I = Cell;

    fn apply(
        &self,
        cell: &NetRef<Self::I>,
        cell_type: &Self::I,
        _create: &Create<Self::I>,
        replace: &mut Replace<Self::I>,
    ) -> Result<bool, Error> {
        if matches!(
            cell_type.get_type(),
            CellType::AND | CellType::AND2 | CellType::OR | CellType::OR2
        ) && let Some(a) = cell.get_input(0).get_driver()
            && let Some(b) = cell.get_input(1).get_driver()
            && a == b
        {
            let c = cell.get_output(0);
            debug!("Pattern applied to cell {}!", c.as_net());
            replace(c, a)?;

            return Ok(true);
        }

        Ok(false)
    }
}

/// Folds monotone AND/OR gate trees (nested homogeneous gates) into a single equivalent (wider) gate.
/// The fold applies to all AND or all OR gates until final total fan-in = 4 (max-sized gate, i.e. AND4)
#[derive(Debug)]
pub struct MonotoneFold;

impl fmt::Display for MonotoneFold {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "AND2(AND2(a,b), c) => AND3(a,b,c)")
    }
}

impl MonotoneFold {
    /// Helper to [Self::apply] that checks homogeneity of fanin gates
    fn can_combine(ct: CellType, children: &[CellType], extra: usize) -> Option<CellType> {
        if !ct.is_and() && !ct.is_or() {
            return None;
        }
        let andg = ct.is_and();
        let mut fanin = extra;
        for child in children {
            if andg != child.is_and() {
                return None;
            }
            fanin += child.get_num_inputs();
        }
        match (andg, fanin) {
            (true, 2) => Some(CellType::AND2),
            (false, 2) => Some(CellType::OR2),
            (true, 3) => Some(CellType::AND3),
            (false, 3) => Some(CellType::OR3),
            (true, 4) => Some(CellType::AND4),
            (false, 4) => Some(CellType::OR4),
            _ => None,
        }
    }
}

/// Return type for [`search_inputs`]: `(const_false_port, const_true_port, other_driver)`.
/// `None` at the outer level means an input was undriven; inner `None` means no constant of
/// that polarity was found.
type ConstInputs = Option<(
    Option<DrivenNet<Cell>>,
    Option<DrivenNet<Cell>>,
    Option<DrivenNet<Cell>>,
)>;

/// Scans the inputs of a gate and classifies each as a constant-false port, constant-true port,
/// or non-constant driver. Returns `None` if any input is undriven.
/// Helper to [`AndAbsorb`], [`AndIdentity`], [`OrAbsorb`], [`OrIdentity`],
/// [`NandAbsorb`], [`NandIdentity`], [`NorAbsorb`], and [`NorIdentity`].
fn search_inputs(cell: &NetRef<Cell>) -> ConstInputs {
    use safety_net::Logic;
    let mut const_false: Option<DrivenNet<Cell>> = None;
    let mut const_true: Option<DrivenNet<Cell>> = None;
    let mut other: Option<DrivenNet<Cell>> = None;
    for port in cell.inputs() {
        let driver = port.get_driver()?;
        match driver.get_instance_type().and_then(|t| t.get_constant()) {
            Some(Logic::False) => {
                const_false = Some(driver);
            }
            Some(Logic::True) => {
                const_true = Some(driver);
            }
            _ => {
                other = Some(driver);
            }
        }
    }
    Some((const_false, const_true, other))
}

impl Pattern for MonotoneFold {
    type I = Cell;

    fn apply(
        &self,
        cell: &NetRef<Self::I>,
        cell_type: &Self::I,
        create: &Create<Self::I>,
        replace: &mut Replace<Self::I>,
    ) -> Result<bool, Error> {
        let root_type = cell_type.get_type();

        if !root_type.is_and() && !root_type.is_or() {
            return Ok(false);
        }

        let mut child_drivers: Vec<NetRef<Self::I>> = Vec::new();
        let mut child_types: Vec<CellType> = Vec::new();
        let mut non_child_drivers: Vec<DrivenNet<Self::I>> = Vec::new();

        for input in cell.inputs() {
            let Some(driver) = input.get_driver() else {
                return Ok(false);
            };
            match driver.get_ptype() {
                Some(ct)
                    if (ct.is_and() && root_type.is_and()) || (ct.is_or() && root_type.is_or()) =>
                {
                    child_drivers.push(driver.clone().unwrap());
                    child_types.push(ct);
                }
                _ => {
                    non_child_drivers.push(driver);
                }
            }
        }

        if child_drivers.is_empty() {
            return Ok(false);
        }

        let Some(new_type) = Self::can_combine(root_type, &child_types, non_child_drivers.len())
        else {
            return Ok(false);
        };

        let new_inst_name = cell.get_instance_name().unwrap() + "_folded".into();
        let new_gate = create(cell_type.new_like(new_type), new_inst_name);

        let mut port_idx = 0;
        for child_ref in &child_drivers {
            for input in child_ref.inputs() {
                if let Some(grandchild) = input.get_driver() {
                    new_gate.get_input(port_idx).connect(grandchild);
                }
                port_idx += 1;
            }
        }

        for driver in non_child_drivers {
            new_gate.get_input(port_idx).connect(driver);
            port_idx += 1;
        }

        let old_output = cell.get_output(0);
        let new_output = new_gate.get_output(0);

        debug!("MonotoneFold applied to cell {}!", old_output.as_net());
        replace(old_output, new_output)?;

        Ok(true)
    }
}

/// A AND 0 = 0 (absorbing element)
#[derive(Debug)]
pub struct AndAbsorb;
impl fmt::Display for AndAbsorb {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "A AND 0 = 0")
    }
}
impl Pattern for AndAbsorb {
    type I = Cell;
    fn apply(
        &self,
        cell: &NetRef<Self::I>,
        cell_type: &Self::I,
        _create: &Create<Self::I>,
        replace: &mut Replace<Self::I>,
    ) -> Result<bool, Error> {
        let ct = cell_type.get_type();
        if !matches!(ct, CellType::AND | CellType::AND2) {
            return Ok(false);
        }
        let Some((Some(gnd_port), _, _)) = search_inputs(cell) else {
            return Ok(false);
        };
        let output = cell.get_output(0);
        debug!("AndAbsorb: AND absorb 0 on cell {}!", output.as_net());
        replace(output, gnd_port)?;
        Ok(true)
    }
}

/// A AND 1 = A (identity element)
#[derive(Debug)]
pub struct AndIdentity;
impl fmt::Display for AndIdentity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "A AND 1 = A")
    }
}
impl Pattern for AndIdentity {
    type I = Cell;
    fn apply(
        &self,
        cell: &NetRef<Self::I>,
        cell_type: &Self::I,
        _create: &Create<Self::I>,
        replace: &mut Replace<Self::I>,
    ) -> Result<bool, Error> {
        let ct = cell_type.get_type();
        if !matches!(ct, CellType::AND | CellType::AND2) {
            return Ok(false);
        }
        let Some((_, Some(_), Some(other_driver))) = search_inputs(cell) else {
            return Ok(false);
        };
        let output = cell.get_output(0);
        debug!("AndIdentity: AND identity 1 on cell {}!", output.as_net());
        replace(output, other_driver)?;
        Ok(true)
    }
}

/// A OR 1 = 1 (absorbing element)
#[derive(Debug)]
pub struct OrAbsorb;
impl fmt::Display for OrAbsorb {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "A OR 1 = 1")
    }
}
impl Pattern for OrAbsorb {
    type I = Cell;
    fn apply(
        &self,
        cell: &NetRef<Self::I>,
        cell_type: &Self::I,
        _create: &Create<Self::I>,
        replace: &mut Replace<Self::I>,
    ) -> Result<bool, Error> {
        let ct = cell_type.get_type();
        if !matches!(ct, CellType::OR | CellType::OR2) {
            return Ok(false);
        }
        let Some((_, Some(vcc_port), _)) = search_inputs(cell) else {
            return Ok(false);
        };
        let output = cell.get_output(0);
        debug!("OrAbsorb: OR absorb 1 on cell {}!", output.as_net());
        replace(output, vcc_port)?;
        Ok(true)
    }
}

/// A OR 0 = A (identity element)
#[derive(Debug)]
pub struct OrIdentity;
impl fmt::Display for OrIdentity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "A OR 0 = A")
    }
}
impl Pattern for OrIdentity {
    type I = Cell;
    fn apply(
        &self,
        cell: &NetRef<Self::I>,
        cell_type: &Self::I,
        _create: &Create<Self::I>,
        replace: &mut Replace<Self::I>,
    ) -> Result<bool, Error> {
        let ct = cell_type.get_type();
        if !matches!(ct, CellType::OR | CellType::OR2) {
            return Ok(false);
        }
        let Some((Some(_), _, Some(other_driver))) = search_inputs(cell) else {
            return Ok(false);
        };
        let output = cell.get_output(0);
        debug!("OrIdentity: OR identity 0 on cell {}!", output.as_net());
        replace(output, other_driver)?;
        Ok(true)
    }
}

/// NOT(NOT(A)) = A, INV(INV(A)) = A
#[derive(Debug)]
pub struct DoubleNegation;

impl fmt::Display for DoubleNegation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "NOT(NOT(A)) = A")
    }
}

impl Pattern for DoubleNegation {
    type I = Cell;

    fn apply(
        &self,
        cell: &NetRef<Self::I>,
        cell_type: &Self::I,
        _create: &Create<Self::I>,
        replace: &mut Replace<Self::I>,
    ) -> Result<bool, Error> {
        let cell_type = cell_type.get_type();
        if !matches!(cell_type, CellType::NOT | CellType::INV) {
            return Ok(false);
        }

        let Some(driver) = cell.get_input(0).get_driver() else {
            return Ok(false);
        };

        let Some(cell_type) = driver.get_ptype() else {
            return Ok(false);
        };

        if !matches!(cell_type, CellType::NOT | CellType::INV) {
            return Ok(false);
        }

        let Some(inner_input) = driver.unwrap().get_input(0).get_driver() else {
            return Ok(false);
        };
        let output = cell.get_output(0);
        debug!("DoubleNegation applied to cell {}!", output.as_net());
        replace(output, inner_input)?;

        Ok(true)
    }
}

/// NAND(A, 0) = 1 (absorbing element)
#[derive(Debug)]
pub struct NandAbsorb;
impl fmt::Display for NandAbsorb {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "NAND(A, 0) = 1")
    }
}
impl Pattern for NandAbsorb {
    type I = Cell;
    fn apply(
        &self,
        cell: &NetRef<Self::I>,
        cell_type: &Self::I,
        create: &Create<Self::I>,
        replace: &mut Replace<Self::I>,
    ) -> Result<bool, Error> {
        let ct = cell_type.get_type();
        if !matches!(ct, CellType::NAND | CellType::NAND2) {
            return Ok(false);
        }
        let Some((Some(_), _, _)) = search_inputs(cell) else {
            return Ok(false);
        };
        let inst_name = cell.get_instance_name().unwrap();
        let vcc = create(
            cell_type.new_like(CellType::VCC),
            inst_name + "_const1".into(),
        );
        let output = cell.get_output(0);
        debug!("NandAbsorb: NAND absorb 0 on {}!", output.as_net());
        replace(output, vcc.get_output(0))?;
        Ok(true)
    }
}

/// NAND(A, 1) = NOT(A) (identity element)
#[derive(Debug)]
pub struct NandIdentity;
impl fmt::Display for NandIdentity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "NAND(A, 1) = NOT(A)")
    }
}
impl Pattern for NandIdentity {
    type I = Cell;
    fn apply(
        &self,
        cell: &NetRef<Self::I>,
        cell_type: &Self::I,
        create: &Create<Self::I>,
        replace: &mut Replace<Self::I>,
    ) -> Result<bool, Error> {
        let ct = cell_type.get_type();
        if !matches!(ct, CellType::NAND | CellType::NAND2) {
            return Ok(false);
        }
        let Some((_, Some(_), Some(other))) = search_inputs(cell) else {
            return Ok(false);
        };
        let inst_name = cell.get_instance_name().unwrap();
        let inv = create(cell_type.new_like(CellType::INV), inst_name + "_inv".into());
        inv.get_input(0).connect(other);
        let output = cell.get_output(0);
        debug!("NandIdentity: NAND(A, 1) = NOT(A) on {}!", output.as_net());
        replace(output, inv.get_output(0))?;
        Ok(true)
    }
}

/// NOR(A, 1) = 0 (absorbing element)
#[derive(Debug)]
pub struct NorAbsorb;
impl fmt::Display for NorAbsorb {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "NOR(A, 1) = 0")
    }
}
impl Pattern for NorAbsorb {
    type I = Cell;
    fn apply(
        &self,
        cell: &NetRef<Self::I>,
        cell_type: &Self::I,
        create: &Create<Self::I>,
        replace: &mut Replace<Self::I>,
    ) -> Result<bool, Error> {
        let ct = cell_type.get_type();
        if !matches!(ct, CellType::NOR | CellType::NOR2) {
            return Ok(false);
        }
        let Some((_, Some(_), _)) = search_inputs(cell) else {
            return Ok(false);
        };
        let inst_name = cell.get_instance_name().unwrap();
        let gnd = create(
            cell_type.new_like(CellType::GND),
            inst_name + "_const0".into(),
        );
        let output = cell.get_output(0);
        debug!("NorAbsorb: NOR absorb 1 on {}!", output.as_net());
        replace(output, gnd.get_output(0))?;
        Ok(true)
    }
}

/// NOR(A, 0) = NOT(A) (identity element)
#[derive(Debug)]
pub struct NorIdentity;
impl fmt::Display for NorIdentity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "NOR(A, 0) = NOT(A)")
    }
}
impl Pattern for NorIdentity {
    type I = Cell;
    fn apply(
        &self,
        cell: &NetRef<Self::I>,
        cell_type: &Self::I,
        create: &Create<Self::I>,
        replace: &mut Replace<Self::I>,
    ) -> Result<bool, Error> {
        let ct = cell_type.get_type();
        if !matches!(ct, CellType::NOR | CellType::NOR2) {
            return Ok(false);
        }
        let Some((Some(_), _, Some(other))) = search_inputs(cell) else {
            return Ok(false);
        };
        let inst_name = cell.get_instance_name().unwrap();
        let inv = create(cell_type.new_like(CellType::INV), inst_name + "_inv".into());
        inv.get_input(0).connect(other);
        let output = cell.get_output(0);
        debug!("NorIdentity: NOR(A, 0) = NOT(A) on {}!", output.as_net());
        replace(output, inv.get_output(0))?;

        Ok(true)
    }
}

// Compound-gate ("techmap") patterns

/// NOT(AND(A,B)) = NAND(A,B)
#[derive(Debug)]
pub struct NotAndMap;

impl fmt::Display for NotAndMap {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "NOT(AND(A,B)) => NAND(A,B)")
    }
}

impl Pattern for NotAndMap {
    type I = Cell;

    fn apply(
        &self,
        cell: &NetRef<Self::I>,
        cell_type: &Self::I,
        create: &Create<Self::I>,
        replace: &mut Replace<Self::I>,
    ) -> Result<bool, Error> {
        if !matches!(cell_type.get_type(), CellType::NOT | CellType::INV) {
            return Ok(false);
        }

        let Some(driver) = cell.get_input(0).get_driver() else {
            return Ok(false);
        };

        let Some(and_type) = driver.get_instance_type().map(|t| t.get_type()) else {
            return Ok(false);
        };

        if !matches!(and_type, CellType::AND | CellType::AND2) {
            return Ok(false);
        }

        let target = if matches!(and_type, CellType::AND) {
            CellType::NAND
        } else {
            CellType::NAND2
        };

        let and_ref = driver.unwrap();

        let Some(a) = and_ref.get_input(0).get_driver() else {
            return Ok(false);
        };

        let Some(b) = and_ref.get_input(1).get_driver() else {
            return Ok(false);
        };

        let inst_name = cell.get_instance_name().unwrap();

        let nand = create(
            cell_type.new_like(target),
            inst_name + "_nand".into(),
        );

        nand.get_input(0).connect(a);
        nand.get_input(1).connect(b);

        let output = cell.get_output(0);
        debug!("NotAndMap: NOT(AND(A,B)) => NAND on {}!", output.as_net());
        replace(output, nand.get_output(0))?;

        Ok(true)
    }
}

/// NOT(OR(A,B)) = NOR(A,B)
#[derive(Debug)]
pub struct NotOrMap;

impl fmt::Display for NotOrMap {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "NOT(OR(A,B)) => NOR(A,B)")
    }
}

impl Pattern for NotOrMap {
    type I = Cell;

    fn apply(
        &self,
        cell: &NetRef<Self::I>,
        cell_type: &Self::I,
        create: &Create<Self::I>,
        replace: &mut Replace<Self::I>,
    ) -> Result<bool, Error> {
        if !matches!(cell_type.get_type(), CellType::NOT | CellType::INV) {
            return Ok(false);
        }

        let Some(driver) = cell.get_input(0).get_driver() else {
            return Ok(false);
        };

        let Some(or_type) = driver.get_instance_type().map(|t| t.get_type()) else {
            return Ok(false);
        };

        if !matches!(or_type, CellType::OR | CellType::OR2) {
            return Ok(false);
        }

        let target = if matches!(or_type, CellType::OR) {
            CellType::NOR
        } else {
            CellType::NOR2
        };

        let or_ref = driver.unwrap();

        let Some(a) = or_ref.get_input(0).get_driver() else {
            return Ok(false);
        };

        let Some(b) = or_ref.get_input(1).get_driver() else {
            return Ok(false);
        };

        let inst_name = cell.get_instance_name().unwrap();

        let nor = create(
            cell_type.new_like(target),
            inst_name + "_nor".into(),
        );

        nor.get_input(0).connect(a);
        nor.get_input(1).connect(b);

        let output = cell.get_output(0);
        debug!("NotOrMap: NOT(OR(A,B)) => NOR on {}!", output.as_net());
        replace(output, nor.get_output(0))?;

        Ok(true)
    }
}

/// NOR(AND(B1,B2), A) = AOI21(A,B1,B2)
#[derive(Debug)]
pub struct AoiMap;

impl fmt::Display for AoiMap {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "NOR(AND(B1,B2),A) => AOI21(A,B1,B2)")
    }
}

impl Pattern for AoiMap {
    type I = Cell;

    fn apply(
        &self,
        cell: &NetRef<Self::I>,
        cell_type: &Self::I,
        create: &Create<Self::I>,
        replace: &mut Replace<Self::I>,
    ) -> Result<bool, Error> {
        if !matches!(cell_type.get_type(), CellType::NOR | CellType::NOR2) {
            return Ok(false);
        }

        let inputs: Vec<_> = cell.inputs().collect();

        if inputs.len() != 2 {
            return Ok(false);
        }

        let Some(d0) = inputs[0].get_driver() else {
            return Ok(false);
        };

        let Some(d1) = inputs[1].get_driver() else {
            return Ok(false);
        };

        let t0 = d0.get_instance_type().map(|t| t.get_type());
        let t1 = d1.get_instance_type().map(|t| t.get_type());

        let is_and = |t: Option<CellType>| {
            matches!(t, Some(CellType::AND) | Some(CellType::AND2))
        };

        let (and_driver, other_driver) = if is_and(t0) {
            (d0, d1)
        } else if is_and(t1) {
            (d1, d0)
        } else {
            return Ok(false);
        };

        let and_ref = and_driver.unwrap();

        let Some(b1) = and_ref.get_input(0).get_driver() else {
            return Ok(false);
        };

        let Some(b2) = and_ref.get_input(1).get_driver() else {
            return Ok(false);
        };

        let inst_name = cell.get_instance_name().unwrap();

        let aoi = create(
            cell_type.new_like(CellType::AOI21),
            inst_name + "_aoi21".into(),
        );

        aoi.get_input(0).connect(other_driver);
        aoi.get_input(1).connect(b1);
        aoi.get_input(2).connect(b2);

        let output = cell.get_output(0);
        debug!("AoiMap: NOR(AND(B1,B2),A) => AOI21 on {}!", output.as_net());
        replace(output, aoi.get_output(0))?;

        Ok(true)
    }
}

/// NAND(OR(B1,B2), A) = OAI21(A,B1,B2)
#[derive(Debug)]
pub struct OaiMap;

impl fmt::Display for OaiMap {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "NAND(OR(B1,B2),A) => OAI21(A,B1,B2)")
    }
}

impl Pattern for OaiMap {
    type I = Cell;

    fn apply(
        &self,
        cell: &NetRef<Self::I>,
        cell_type: &Self::I,
        create: &Create<Self::I>,
        replace: &mut Replace<Self::I>,
    ) -> Result<bool, Error> {
        if !matches!(cell_type.get_type(), CellType::NAND | CellType::NAND2) {
            return Ok(false);
        }

        let inputs: Vec<_> = cell.inputs().collect();

        if inputs.len() != 2 {
            return Ok(false);
        }

        let Some(d0) = inputs[0].get_driver() else {
            return Ok(false);
        };

        let Some(d1) = inputs[1].get_driver() else {
            return Ok(false);
        };

        let t0 = d0.get_instance_type().map(|t| t.get_type());
        let t1 = d1.get_instance_type().map(|t| t.get_type());

        let is_or = |t: Option<CellType>| {
            matches!(t, Some(CellType::OR) | Some(CellType::OR2))
        };

        let (or_driver, other_driver) = if is_or(t0) {
            (d0, d1)
        } else if is_or(t1) {
            (d1, d0)
        } else {
            return Ok(false);
        };

        let or_ref = or_driver.unwrap();

        let Some(b1) = or_ref.get_input(0).get_driver() else {
            return Ok(false);
        };

        let Some(b2) = or_ref.get_input(1).get_driver() else {
            return Ok(false);
        };

        let inst_name = cell.get_instance_name().unwrap();

        let oai = create(
            cell_type.new_like(CellType::OAI21),
            inst_name + "_oai21".into(),
        );

        oai.get_input(0).connect(other_driver);
        oai.get_input(1).connect(b1);
        oai.get_input(2).connect(b2);

        let output = cell.get_output(0);
        debug!("OaiMap: NAND(OR(B1,B2),A) => OAI21 on {}!", output.as_net());
        replace(output, oai.get_output(0))?;

        Ok(true)
    }
}

/// NOR(AND(A1,A2), AND(B1,B2)) = AOI22(A1,A2,B1,B2)
#[derive(Debug)]
pub struct AoiMap22;

impl fmt::Display for AoiMap22 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "NOR(AND(A1,A2),AND(B1,B2)) => AOI22(A1,A2,B1,B2)"
        )
    }
}

impl Pattern for AoiMap22 {
    type I = Cell;

    fn apply(
        &self,
        cell: &NetRef<Self::I>,
        cell_type: &Self::I,
        create: &Create<Self::I>,
        replace: &mut Replace<Self::I>,
    ) -> Result<bool, Error> {
        if !matches!(cell_type.get_type(), CellType::NOR | CellType::NOR2) {
            return Ok(false);
        }

        let inputs: Vec<_> = cell.inputs().collect();

        if inputs.len() != 2 {
            return Ok(false);
        }

        let Some(d0) = inputs[0].get_driver() else {
            return Ok(false);
        };

        let Some(d1) = inputs[1].get_driver() else {
            return Ok(false);
        };

        let is_and = |d: &DrivenNet<Cell>| {
            matches!(
                d.get_instance_type().map(|t| t.get_type()),
                Some(CellType::AND) | Some(CellType::AND2)
            )
        };

        if !is_and(&d0) || !is_and(&d1) {
            return Ok(false);
        }

        let r0 = d0.unwrap();
        let r1 = d1.unwrap();

        let (Some(a1), Some(a2)) = (
            r0.get_input(0).get_driver(),
            r0.get_input(1).get_driver(),
        ) else {
            return Ok(false);
        };

        let (Some(b1), Some(b2)) = (
            r1.get_input(0).get_driver(),
            r1.get_input(1).get_driver(),
        ) else {
            return Ok(false);
        };

        let inst_name = cell.get_instance_name().unwrap();

        let aoi = create(
            cell_type.new_like(CellType::AOI22),
            inst_name + "_aoi22".into(),
        );

        aoi.get_input(0).connect(a1);
        aoi.get_input(1).connect(a2);
        aoi.get_input(2).connect(b1);
        aoi.get_input(3).connect(b2);

        let output = cell.get_output(0);
        debug!("AoiMap22: NOR(AND,AND) => AOI22 on {}!", output.as_net());
        replace(output, aoi.get_output(0))?;

        Ok(true)
    }
}

/// NAND(OR(A1,A2), OR(B1,B2)) = OAI22(A1,A2,B1,B2)
#[derive(Debug)]
pub struct OaiMap22;

impl fmt::Display for OaiMap22 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "NAND(OR(A1,A2),OR(B1,B2)) => OAI22(A1,A2,B1,B2)"
        )
    }
}

impl Pattern for OaiMap22 {
    type I = Cell;

    fn apply(
        &self,
        cell: &NetRef<Self::I>,
        cell_type: &Self::I,
        create: &Create<Self::I>,
        replace: &mut Replace<Self::I>,
    ) -> Result<bool, Error> {
        if !matches!(cell_type.get_type(), CellType::NAND | CellType::NAND2) {
            return Ok(false);
        }

        let inputs: Vec<_> = cell.inputs().collect();

        if inputs.len() != 2 {
            return Ok(false);
        }

        let Some(d0) = inputs[0].get_driver() else {
            return Ok(false);
        };

        let Some(d1) = inputs[1].get_driver() else {
            return Ok(false);
        };

        let is_or = |d: &DrivenNet<Cell>| {
            matches!(
                d.get_instance_type().map(|t| t.get_type()),
                Some(CellType::OR) | Some(CellType::OR2)
            )
        };

        if !is_or(&d0) || !is_or(&d1) {
            return Ok(false);
        }

        let r0 = d0.unwrap();
        let r1 = d1.unwrap();

        let (Some(a1), Some(a2)) = (
            r0.get_input(0).get_driver(),
            r0.get_input(1).get_driver(),
        ) else {
            return Ok(false);
        };

        let (Some(b1), Some(b2)) = (
            r1.get_input(0).get_driver(),
            r1.get_input(1).get_driver(),
        ) else {
            return Ok(false);
        };

        let inst_name = cell.get_instance_name().unwrap();

        let oai = create(
            cell_type.new_like(CellType::OAI22),
            inst_name + "_oai22".into(),
        );

        oai.get_input(0).connect(a1);
        oai.get_input(1).connect(a2);
        oai.get_input(2).connect(b1);
        oai.get_input(3).connect(b2);

        let output = cell.get_output(0);
        debug!("OaiMap22: NAND(OR,OR) => OAI22 on {}!", output.as_net());
        replace(output, oai.get_output(0))?;

        Ok(true)
    }
}

/// MUX(S, A, A) = A
#[derive(Debug)]
pub struct MuxSameInput;

impl fmt::Display for MuxSameInput {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "MUX(S,A,A) = A")
    }
}

impl Pattern for MuxSameInput {
    type I = Cell;

    fn apply(
        &self,
        cell: &NetRef<Self::I>,
        cell_type: &Self::I,
        _create: &Create<Self::I>,
        replace: &mut Replace<Self::I>,
    ) -> Result<bool, Error> {
        if !matches!(cell_type.get_type(), CellType::MUX | CellType::MUX2) {
            return Ok(false);
        }

        let Some(d1) = cell.get_input(1).get_driver() else {
            return Ok(false);
        };

        let Some(d2) = cell.get_input(2).get_driver() else {
            return Ok(false);
        };

        if d1 != d2 {
            return Ok(false);
        }

        let output = cell.get_output(0);
        debug!("MuxSameInput applied to cell {}!", output.as_net());
        replace(output, d1)?;

        Ok(true)
    }
}

/// MUX(S,A,B) with constant S selects A or B directly.
#[derive(Debug)]
pub struct MuxConstSelect;

impl fmt::Display for MuxConstSelect {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "MUX(const S,A,B) = A or B")
    }
}

impl Pattern for MuxConstSelect {
    type I = Cell;

    fn apply(
        &self,
        cell: &NetRef<Self::I>,
        cell_type: &Self::I,
        _create: &Create<Self::I>,
        replace: &mut Replace<Self::I>,
    ) -> Result<bool, Error> {
        use safety_net::Logic;

        let ct = cell_type.get_type();

        let (a_idx, b_idx) = match ct {
            CellType::MUX => (1, 2),
            CellType::MUX2 => (2, 1),
            _ => return Ok(false),
        };

        let Some(s) = cell.get_input(0).get_driver() else {
            return Ok(false);
        };

        let Some(sel) = s.get_instance_type().and_then(|t| t.get_constant()) else {
            return Ok(false);
        };

        let chosen_idx = match sel {
            Logic::True => a_idx,
            Logic::False => b_idx,
            _ => return Ok(false),
        };

        let Some(chosen) = cell.get_input(chosen_idx).get_driver() else {
            return Ok(false);
        };

        let output = cell.get_output(0);

        debug!(
            "MuxConstSelect: constant-select MUX resolved on {}!",
            output.as_net()
        );

        replace(output, chosen)?;

        Ok(true)
    }
}
