use safety_net::{Instantiable, Net, Netlist};
use safety_pass::patterns::{
    AndAbsorb, AndIdentity, AoiMap, AoiMap22, DoubleNegation, Idempotent, MonotoneFold,
    MuxConstSelect, MuxSameInput, NandAbsorb, NandIdentity, NorAbsorb, NorIdentity, NotAndMap,
    NotOrMap, OaiMap, OaiMap22, OrAbsorb, OrIdentity,
};
use safety_pass::{Cell, CellType, Folder, Pass, Primitive};
use std::rc::Rc;

fn and_gate() -> Cell {
    Cell::new(CellType::AND2, None)
}

fn ex_netlist() -> Rc<Netlist<Cell>> {
    let nl = Netlist::new("top".into());
    let a = nl.insert_input(Net::new_logic("a".into()));
    let b = nl.insert_input(Net::new_logic("b".into()));
    let g = nl
        .insert_gate(and_gate(), "inst_0".into(), &[a, b])
        .unwrap()
        .get_output(0);
    let h = nl
        .insert_gate(and_gate(), "inst_1".into(), &[g.clone(), g])
        .unwrap();

    h.expose_with_name("y".into());

    nl
}

#[test]
fn test_ld_pattern() {
    let nl = ex_netlist();

    let mut folder = Folder::<Cell>::new(101);
    folder.insert(Idempotent);

    let before = nl.len();

    let res = folder.run(&nl);
    assert!(res.is_ok());

    let after = nl.len();
    assert_eq!(after + 1, before);

    assert!(res.unwrap().contains("1 iterations"));
}

#[test]
fn test_run_twice_pattern() {
    let nl = ex_netlist();

    let mut folder = Folder::<Cell>::new(101);
    folder.insert(Idempotent);

    let before = nl.len();

    let res = folder.run(&nl);
    assert!(res.is_ok());

    let after = nl.len();
    assert_eq!(after + 1, before);

    let res = folder.run(&nl);
    assert!(res.is_ok());

    let fin = nl.len();

    assert_eq!(fin, after);

    assert!(res.unwrap().contains("0 iterations"));
}

fn or_gate() -> Cell {
    Cell::new(CellType::OR2, None)
}

fn monotone_and_netlist() -> Rc<Netlist<Cell>> {
    // a ─┐
    //    AND2(inst_0) ─┐
    // b ─┘             ├── AND2(inst_1) ── y
    // c ───────────────┘
    let nl = Netlist::new("top".into());
    let a = nl.insert_input(Net::new_logic("a".into()));
    let b = nl.insert_input(Net::new_logic("b".into()));
    let c = nl.insert_input(Net::new_logic("c".into()));
    let g = nl
        .insert_gate(and_gate(), "inst_0".into(), &[a, b])
        .unwrap()
        .get_output(0);
    let h = nl
        .insert_gate(and_gate(), "inst_1".into(), &[g, c])
        .unwrap();
    h.expose_with_name("y".into());
    nl
}

fn monotone_and4_netlist() -> Rc<Netlist<Cell>> {
    // a ─┐
    //    AND2(inst_0) ─┐
    // b ─┘             ├── AND2(inst_2) ── y
    // c ─┐             │
    //    AND2(inst_1) ─┘
    // d ─┘
    let nl = Netlist::new("top".into());
    let a = nl.insert_input(Net::new_logic("a".into()));
    let b = nl.insert_input(Net::new_logic("b".into()));
    let c = nl.insert_input(Net::new_logic("c".into()));
    let d = nl.insert_input(Net::new_logic("d".into()));
    let g = nl
        .insert_gate(and_gate(), "inst_0".into(), &[a, b])
        .unwrap()
        .get_output(0);
    let h = nl
        .insert_gate(and_gate(), "inst_1".into(), &[c, d])
        .unwrap()
        .get_output(0);
    let top = nl
        .insert_gate(and_gate(), "inst_2".into(), &[g, h])
        .unwrap();
    top.expose_with_name("y".into());
    nl
}

fn monotone_or_netlist() -> Rc<Netlist<Cell>> {
    // Same shape as monotone_and_netlist but with OR gates
    let nl = Netlist::new("top".into());
    let a = nl.insert_input(Net::new_logic("a".into()));
    let b = nl.insert_input(Net::new_logic("b".into()));
    let c = nl.insert_input(Net::new_logic("c".into()));
    let g = nl
        .insert_gate(or_gate(), "inst_0".into(), &[a, b])
        .unwrap()
        .get_output(0);
    let h = nl.insert_gate(or_gate(), "inst_1".into(), &[g, c]).unwrap();
    h.expose_with_name("y".into());
    nl
}

fn monotone_no_fold_netlist() -> Rc<Netlist<Cell>> {
    // a ─┐
    //    AND2(inst_0) ─┬── AND2(inst_1) ── y1
    // b ─┘             └── AND2(inst_2) ── y2
    // c ───────────────┘
    // d ───────────────┘
    // after merge
    // a ─┬── AND3(inst_2_folded) ── y2
    // b ─┤
    // d ─┘
    let nl = Netlist::new("top".into());
    let a = nl.insert_input(Net::new_logic("a".into()));
    let b = nl.insert_input(Net::new_logic("b".into()));
    let c = nl.insert_input(Net::new_logic("c".into()));
    let d = nl.insert_input(Net::new_logic("d".into()));
    let g = nl
        .insert_gate(and_gate(), "inst_0".into(), &[a, b])
        .unwrap()
        .get_output(0);
    let h1 = nl
        .insert_gate(and_gate(), "inst_1".into(), &[g.clone(), c])
        .unwrap();
    let h2 = nl
        .insert_gate(and_gate(), "inst_2".into(), &[g, d])
        .unwrap();
    h1.expose_with_name("y1".into());
    h2.expose_with_name("y2".into());
    nl
}

fn and_const0_netlist() -> Rc<Netlist<Cell>> {
    // AND(a, 0) = 0
    let nl = Netlist::new("top".into());
    let a = nl.insert_input(Net::new_logic("a".into()));
    let gnd = nl
        .insert_constant(safety_net::Logic::False, "gnd".into())
        .unwrap();
    let g = nl
        .insert_gate(and_gate(), "inst_0".into(), &[a, gnd])
        .unwrap();
    g.expose_with_name("y".into());
    nl
}

fn and_const1_netlist() -> Rc<Netlist<Cell>> {
    // AND(a, 1) = a
    let nl = Netlist::new("top".into());
    let a = nl.insert_input(Net::new_logic("a".into()));
    let vcc = nl
        .insert_constant(safety_net::Logic::True, "vcc".into())
        .unwrap();
    let g = nl
        .insert_gate(and_gate(), "inst_0".into(), &[a, vcc])
        .unwrap();
    g.expose_with_name("y".into());
    nl
}

fn or_const1_netlist() -> Rc<Netlist<Cell>> {
    // OR(a, 1) = 1
    let nl = Netlist::new("top".into());
    let a = nl.insert_input(Net::new_logic("a".into()));
    let vcc = nl
        .insert_constant(safety_net::Logic::True, "vcc".into())
        .unwrap();
    let g = nl
        .insert_gate(or_gate(), "inst_0".into(), &[a, vcc])
        .unwrap();
    g.expose_with_name("y".into());
    nl
}

fn or_const0_netlist() -> Rc<Netlist<Cell>> {
    // OR(a, 0) = a
    let nl = Netlist::new("top".into());
    let a = nl.insert_input(Net::new_logic("a".into()));
    let gnd = nl
        .insert_constant(safety_net::Logic::False, "gnd".into())
        .unwrap();
    let g = nl
        .insert_gate(or_gate(), "inst_0".into(), &[a, gnd])
        .unwrap();
    g.expose_with_name("y".into());
    nl
}

fn double_neg_netlist() -> Rc<Netlist<Cell>> {
    // NOT(NOT(a)) = a
    let nl = Netlist::new("top".into());
    let a = nl.insert_input(Net::new_logic("a".into()));
    let inv1 = nl
        .insert_gate(Cell::new(CellType::INV, None), "inv1".into(), &[a])
        .unwrap()
        .get_output(0);
    let inv2 = nl
        .insert_gate(Cell::new(CellType::INV, None), "inv2".into(), &[inv1])
        .unwrap();
    inv2.expose_with_name("y".into());
    nl
}

fn double_neg_not_netlist() -> Rc<Netlist<Cell>> {
    // NOT(NOT(a)) using NOT cells
    let nl = Netlist::new("top".into());
    let a = nl.insert_input(Net::new_logic("a".into()));
    let not1 = nl
        .insert_gate(Cell::new(CellType::NOT, None), "not1".into(), &[a])
        .unwrap()
        .get_output(0);
    let not2 = nl
        .insert_gate(Cell::new(CellType::NOT, None), "not2".into(), &[not1])
        .unwrap();
    not2.expose_with_name("y".into());
    nl
}

fn single_inv_netlist() -> Rc<Netlist<Cell>> {
    // Single INV — should NOT fire
    let nl = Netlist::new("top".into());
    let a = nl.insert_input(Net::new_logic("a".into()));
    let inv = nl
        .insert_gate(Cell::new(CellType::INV, None), "inv1".into(), &[a])
        .unwrap();
    inv.expose_with_name("y".into());
    nl
}

fn nand_const0_netlist() -> Rc<Netlist<Cell>> {
    // NAND(a, 0) = 1
    let nl = Netlist::new("top".into());
    let a = nl.insert_input(Net::new_logic("a".into()));
    let gnd = nl
        .insert_constant(safety_net::Logic::False, "gnd".into())
        .unwrap();
    let g = nl
        .insert_gate(Cell::new(CellType::NAND2, None), "inst_0".into(), &[a, gnd])
        .unwrap();
    g.expose_with_name("y".into());
    nl
}

fn nand_const1_netlist() -> Rc<Netlist<Cell>> {
    // NAND(a, 1) = NOT(a)
    let nl = Netlist::new("top".into());
    let a = nl.insert_input(Net::new_logic("a".into()));
    let vcc = nl
        .insert_constant(safety_net::Logic::True, "vcc".into())
        .unwrap();
    let g = nl
        .insert_gate(Cell::new(CellType::NAND2, None), "inst_0".into(), &[a, vcc])
        .unwrap();
    g.expose_with_name("y".into());
    nl
}

fn nor_const1_netlist() -> Rc<Netlist<Cell>> {
    // NOR(a, 1) = 0
    let nl = Netlist::new("top".into());
    let a = nl.insert_input(Net::new_logic("a".into()));
    let vcc = nl
        .insert_constant(safety_net::Logic::True, "vcc".into())
        .unwrap();
    let g = nl
        .insert_gate(Cell::new(CellType::NOR2, None), "inst_0".into(), &[a, vcc])
        .unwrap();
    g.expose_with_name("y".into());
    nl
}

fn nor_const0_netlist() -> Rc<Netlist<Cell>> {
    // NOR(a, 0) = NOT(a)
    let nl = Netlist::new("top".into());
    let a = nl.insert_input(Net::new_logic("a".into()));
    let gnd = nl
        .insert_constant(safety_net::Logic::False, "gnd".into())
        .unwrap();
    let g = nl
        .insert_gate(Cell::new(CellType::NOR2, None), "inst_0".into(), &[a, gnd])
        .unwrap();
    g.expose_with_name("y".into());
    nl
}

#[test]
fn test_monotone_fold_and3() {
    // AND2(AND2(a,b), c) => AND3(a,b,c)
    // before: 5 objects (a, b, c, inst_0, inst_1)
    // after:  4 objects (a, b, c, inst_1_folded), inst_0 orphaned and cleaned
    let nl = monotone_and_netlist();
    let mut folder = Folder::<Cell>::new(101);
    folder.insert(MonotoneFold);
    let before = nl.len();
    assert_eq!(before, 5);
    let res = folder.run(&nl);
    assert!(res.is_ok());
    let after = nl.len();
    assert_eq!(after + 1, before);
    // Verify the remaining gate is AND3
    let gates: Vec<_> = nl.objects().collect();
    assert_eq!(gates.len(), 4);
    assert_eq!(gates[3].get_ptype(), Some(CellType::AND3));
}

#[test]
fn test_monotone_fold_and4() {
    // AND2(AND2(a,b), AND2(c,d)) => AND4(a,b,c,d)
    // before: 7 objects (a, b, c, d, inst_0, inst_1, inst_2)
    // after:  5 objects (a, b, c, d, inst_2_folded), inst_0 and inst_1 cleaned
    let nl = monotone_and4_netlist();
    let mut folder = Folder::<Cell>::new(101);
    folder.insert(MonotoneFold);
    let before = nl.len();
    assert_eq!(before, 7);
    let res = folder.run(&nl);
    assert!(res.is_ok());
    let after = nl.len();
    assert_eq!(after + 2, before);
    let gates: Vec<_> = nl.objects().collect();
    assert_eq!(gates.len(), 5);
    assert_eq!(gates[4].get_ptype(), Some(CellType::AND4));
}

#[test]
fn test_monotone_fold_or3() {
    // OR2(OR2(a,b), c) => OR3(a,b,c)
    // before 5 objects, after 4 objects
    let nl = monotone_or_netlist();
    let mut folder = Folder::<Cell>::new(101);
    folder.insert(MonotoneFold);
    let before = nl.len();
    assert_eq!(before, 5);
    let res = folder.run(&nl);
    assert!(res.is_ok());
    let after = nl.len();
    assert_eq!(after + 1, before);
    let gates: Vec<_> = nl.objects().collect();
    assert_eq!(gates.len(), 4);
    assert_eq!(gates[3].get_ptype(), Some(CellType::OR3));
}

#[test]
fn test_monotone_fold_idempotent_after() {
    let nl = monotone_and_netlist();
    let mut folder = Folder::<Cell>::new(101);
    folder.insert(MonotoneFold);
    let res1 = folder.run(&nl);
    assert!(res1.is_ok());
    let after_first = nl.len();
    let res2 = folder.run(&nl);
    assert!(res2.is_ok());
    assert_eq!(nl.len(), after_first);
    assert!(res2.unwrap().contains("0 iterations"))
}

#[test]
fn test_monotone_no_fold_shared_child() {
    let nl = monotone_no_fold_netlist();
    let mut folder = Folder::<Cell>::new(101);
    folder.insert(MonotoneFold);
    let before = nl.len();
    let res = folder.run(&nl);
    assert!(res.is_ok());
    assert_eq!(nl.len(), before - 1);
    assert!(res.unwrap().contains("2 iterations"));
}

#[test]
fn test_constant_fold_and0() {
    // AND(a, 0) = 0
    // before: 3 objects (a, gnd, inst_0)
    // after:  2 objects (a, gnd) inst_0 cleaned, output rewired to gnd
    let nl = and_const0_netlist();
    let mut folder = Folder::<Cell>::new(101);
    folder.insert(AndAbsorb);
    let before = nl.len();
    assert_eq!(before, 3);
    let res = folder.run(&nl);
    assert!(res.is_ok());
    assert_eq!(nl.len(), before - 1);
    // verify output is driven by a constant false
    let outputs = nl.outputs();
    assert_eq!(outputs.len(), 1);
    let driver = outputs[0].0.clone().unwrap();
    assert_eq!(
        driver.get_instance_type().unwrap().get_constant(),
        Some(safety_net::Logic::False)
    );
}

#[test]
fn test_constant_fold_and1() {
    // AND(a, 1) = a
    // before: 3 objects (a, vcc, inst_0)
    // after:  2 objects (a, vcc) inst_0 cleaned, output rewired to a
    let nl = and_const1_netlist();
    let mut folder = Folder::<Cell>::new(101);
    folder.insert(AndIdentity);
    let before = nl.len();
    assert_eq!(before, 3);
    let res = folder.run(&nl);
    assert!(res.is_ok());
    assert_eq!(nl.len(), before - 2);
    // verify output is driven by the input a
    let outputs = nl.outputs();
    assert_eq!(outputs.len(), 1);
    assert!(outputs[0].0.is_an_input());
}

#[test]
fn test_constant_fold_or1() {
    // OR(a, 1) = 1
    let nl = or_const1_netlist();
    let mut folder = Folder::<Cell>::new(101);
    folder.insert(OrAbsorb);
    let before = nl.len();
    assert_eq!(before, 3);
    let res = folder.run(&nl);
    assert!(res.is_ok());
    assert_eq!(nl.len(), before - 1);
    let outputs = nl.outputs();
    assert_eq!(outputs.len(), 1);
    let driver = outputs[0].0.clone().unwrap();
    assert_eq!(
        driver.get_instance_type().unwrap().get_constant(),
        Some(safety_net::Logic::True)
    );
}

#[test]
fn test_constant_fold_or0() {
    // OR(a, 0) = a
    let nl = or_const0_netlist();
    let mut folder = Folder::<Cell>::new(101);
    folder.insert(OrIdentity);
    let before = nl.len();
    assert_eq!(before, 3);
    let res = folder.run(&nl);
    assert!(res.is_ok());
    assert_eq!(nl.len(), before - 2);
    let outputs = nl.outputs();
    assert_eq!(outputs.len(), 1);
    assert!(outputs[0].0.is_an_input());
}

#[test]
fn test_double_negation_inv() {
    // INV(INV(a)) = a
    // before: 3 objects (a, inv1, inv2)
    // after:  1 object  (a) both inverters cleaned
    let nl = double_neg_netlist();
    let mut folder = Folder::<Cell>::new(101);
    folder.insert(DoubleNegation);
    let before = nl.len();
    assert_eq!(before, 3);
    let res = folder.run(&nl);
    assert!(res.is_ok());
    assert_eq!(nl.len(), 1);
    // output should now be driven directly by input a
    let outputs = nl.outputs();
    assert_eq!(outputs.len(), 1);
    assert!(outputs[0].0.is_an_input());
}

#[test]
fn test_double_negation_not() {
    // NOT(NOT(a)) = a
    let nl = double_neg_not_netlist();
    let mut folder = Folder::<Cell>::new(101);
    folder.insert(DoubleNegation);
    let before = nl.len();
    assert_eq!(before, 3);
    let res = folder.run(&nl);
    assert!(res.is_ok());
    assert_eq!(nl.len(), 1);
    let outputs = nl.outputs();
    assert_eq!(outputs.len(), 1);
    assert!(outputs[0].0.is_an_input());
}

#[test]
fn test_double_negation_no_fire_single() {
    // Single INV should not be touched
    let nl = single_inv_netlist();
    let mut folder = Folder::<Cell>::new(101);
    folder.insert(DoubleNegation);
    let before = nl.len();
    let res = folder.run(&nl);
    assert!(res.is_ok());
    assert_eq!(nl.len(), before);
    assert!(res.unwrap().contains("0 iterations"));
}

#[test]
fn test_nand_const0() {
    // NAND(a, 0) = 1 output rewired to new VCC
    let nl = nand_const0_netlist();
    let mut folder = Folder::<Cell>::new(101);
    folder.insert(NandAbsorb);
    let res = folder.run(&nl);
    assert!(res.is_ok());
    let outputs = nl.outputs();
    assert_eq!(outputs.len(), 1);
    let driver = outputs[0].0.clone().unwrap();
    assert_eq!(
        driver.get_instance_type().unwrap().get_constant(),
        Some(safety_net::Logic::True)
    );
}

#[test]
fn test_nand_const1() {
    // NAND(a, 1) = NOT(a) output rewired to new INV
    let nl = nand_const1_netlist();
    let mut folder = Folder::<Cell>::new(101);
    folder.insert(NandIdentity);
    let res = folder.run(&nl);
    assert!(res.is_ok());
    let outputs = nl.outputs();
    assert_eq!(outputs.len(), 1);
    let driver = outputs[0].0.clone().unwrap();
    assert_eq!(driver.get_ptype(), Some(CellType::INV));
    let inv_input = driver.get_input(0).get_driver().unwrap();
    let inputs: Vec<_> = nl.inputs().collect();
    assert_eq!(inputs.len(), 1);
    assert_eq!(inv_input, inputs[0]);
}

#[test]
fn test_nor_const1() {
    // NOR(a, 1) = 0 output rewired to new GND
    let nl = nor_const1_netlist();
    let mut folder = Folder::<Cell>::new(101);
    folder.insert(NorAbsorb);
    let res = folder.run(&nl);
    assert!(res.is_ok());
    let outputs = nl.outputs();
    assert_eq!(outputs.len(), 1);
    let driver = outputs[0].0.clone().unwrap();
    assert_eq!(
        driver.get_instance_type().unwrap().get_constant(),
        Some(safety_net::Logic::False)
    );
}

#[test]
fn test_nor_const0() {
    // NOR(a, 0) = NOT(a) output rewired to new INV
    let nl = nor_const0_netlist();
    let mut folder = Folder::<Cell>::new(101);
    folder.insert(NorIdentity);
    let res = folder.run(&nl);
    assert!(res.is_ok());
    let outputs = nl.outputs();
    assert_eq!(outputs.len(), 1);
    let driver = outputs[0].0.clone().unwrap();
    assert_eq!(driver.get_ptype(), Some(CellType::INV));
}

fn not_and_netlist() -> Rc<Netlist<Cell>> {
    // NOT(AND(a,b)) = NAND(a,b)
    let nl = Netlist::new("top".into());
    let a = nl.insert_input(Net::new_logic("a".into()));
    let b = nl.insert_input(Net::new_logic("b".into()));

    let and = nl
        .insert_gate(and_gate(), "and".into(), &[a, b])
        .unwrap()
        .get_output(0);

    let not = nl
        .insert_gate(Cell::new(CellType::INV, None), "inv".into(), &[and])
        .unwrap();

    not.expose_with_name("y".into());
    nl
}

fn not_or_netlist() -> Rc<Netlist<Cell>> {
    // NOT(OR(a,b)) = NOR(a,b)
    let nl = Netlist::new("top".into());
    let a = nl.insert_input(Net::new_logic("a".into()));
    let b = nl.insert_input(Net::new_logic("b".into()));

    let or = nl
        .insert_gate(or_gate(), "or".into(), &[a, b])
        .unwrap()
        .get_output(0);

    let not = nl
        .insert_gate(Cell::new(CellType::INV, None), "inv".into(), &[or])
        .unwrap();

    not.expose_with_name("y".into());
    nl
}

fn aoi21_netlist() -> Rc<Netlist<Cell>> {
    // NOR(AND(b1,b2),a) = AOI21(a,b1,b2)
    let nl = Netlist::new("top".into());
    let a = nl.insert_input(Net::new_logic("a".into()));
    let b1 = nl.insert_input(Net::new_logic("b1".into()));
    let b2 = nl.insert_input(Net::new_logic("b2".into()));

    let and = nl
        .insert_gate(and_gate(), "and".into(), &[b1, b2])
        .unwrap()
        .get_output(0);

    let nor = nl
        .insert_gate(Cell::new(CellType::NOR2, None), "nor".into(), &[and, a])
        .unwrap();

    nor.expose_with_name("y".into());
    nl
}

fn oai21_netlist() -> Rc<Netlist<Cell>> {
    // NAND(OR(b1,b2),a) = OAI21(a,b1,b2)
    let nl = Netlist::new("top".into());
    let a = nl.insert_input(Net::new_logic("a".into()));
    let b1 = nl.insert_input(Net::new_logic("b1".into()));
    let b2 = nl.insert_input(Net::new_logic("b2".into()));

    let or = nl
        .insert_gate(or_gate(), "or".into(), &[b1, b2])
        .unwrap()
        .get_output(0);

    let nand = nl
        .insert_gate(
            Cell::new(CellType::NAND2, None),
            "nand".into(),
            &[or, a],
        )
        .unwrap();

    nand.expose_with_name("y".into());
    nl
}

fn aoi22_netlist() -> Rc<Netlist<Cell>> {
    // NOR(AND(a1,a2),AND(b1,b2)) = AOI22(a1,a2,b1,b2)
    let nl = Netlist::new("top".into());
    let a1 = nl.insert_input(Net::new_logic("a1".into()));
    let a2 = nl.insert_input(Net::new_logic("a2".into()));
    let b1 = nl.insert_input(Net::new_logic("b1".into()));
    let b2 = nl.insert_input(Net::new_logic("b2".into()));

    let and1 = nl
        .insert_gate(and_gate(), "and1".into(), &[a1, a2])
        .unwrap()
        .get_output(0);

    let and2 = nl
        .insert_gate(and_gate(), "and2".into(), &[b1, b2])
        .unwrap()
        .get_output(0);

    let nor = nl
        .insert_gate(
            Cell::new(CellType::NOR2, None),
            "nor".into(),
            &[and1, and2],
        )
        .unwrap();

    nor.expose_with_name("y".into());
    nl
}

fn oai22_netlist() -> Rc<Netlist<Cell>> {
    // NAND(OR(a1,a2),OR(b1,b2)) = OAI22(a1,a2,b1,b2)
    let nl = Netlist::new("top".into());
    let a1 = nl.insert_input(Net::new_logic("a1".into()));
    let a2 = nl.insert_input(Net::new_logic("a2".into()));
    let b1 = nl.insert_input(Net::new_logic("b1".into()));
    let b2 = nl.insert_input(Net::new_logic("b2".into()));

    let or1 = nl
        .insert_gate(or_gate(), "or1".into(), &[a1, a2])
        .unwrap()
        .get_output(0);

    let or2 = nl
        .insert_gate(or_gate(), "or2".into(), &[b1, b2])
        .unwrap()
        .get_output(0);

    let nand = nl
        .insert_gate(
            Cell::new(CellType::NAND2, None),
            "nand".into(),
            &[or1, or2],
        )
        .unwrap();

    nand.expose_with_name("y".into());
    nl
}

fn mux_same_input_netlist() -> Rc<Netlist<Cell>> {
    // MUX(s,a,a) = a
    let nl = Netlist::new("top".into());
    let s = nl.insert_input(Net::new_logic("s".into()));
    let a = nl.insert_input(Net::new_logic("a".into()));

    let mux = nl
        .insert_gate(
            Cell::new(CellType::MUX, None),
            "mux".into(),
            &[s, a.clone(), a],
        )
        .unwrap();

    mux.expose_with_name("y".into());
    nl
}

fn mux_const_select_netlist() -> Rc<Netlist<Cell>> {
    // MUX(1,a,b) = a
    let nl = Netlist::new("top".into());
    let a = nl.insert_input(Net::new_logic("a".into()));
    let b = nl.insert_input(Net::new_logic("b".into()));
    let vcc = nl
        .insert_constant(safety_net::Logic::True, "vcc".into())
        .unwrap();

    let mux = nl
        .insert_gate(
            Cell::new(CellType::MUX, None),
            "mux".into(),
            &[vcc, a, b],
        )
        .unwrap();

    mux.expose_with_name("y".into());
    nl
}

#[test]
fn test_not_and_map() {
    // NOT(AND(a,b)) = NAND(a,b)
    let nl = not_and_netlist();
    let mut folder = Folder::<Cell>::new(101);
    folder.insert(NotAndMap);

    let before = nl.len();
    assert_eq!(before, 4);

    let res = folder.run(&nl);
    assert!(res.is_ok());

    assert_eq!(nl.len(), before - 1);

    let outputs = nl.outputs();
    assert_eq!(outputs.len(), 1);

    let driver = outputs[0].0.clone().unwrap();
    assert_eq!(
        driver.get_instance_type().unwrap().get_type(),
        CellType::NAND2
    );
}

#[test]
fn test_not_or_map() {
    // NOT(OR(a,b)) = NOR(a,b)
    let nl = not_or_netlist();
    let mut folder = Folder::<Cell>::new(101);
    folder.insert(NotOrMap);

    let before = nl.len();
    assert_eq!(before, 4);

    let res = folder.run(&nl);
    assert!(res.is_ok());

    assert_eq!(nl.len(), before - 1);

    let outputs = nl.outputs();
    assert_eq!(outputs.len(), 1);

    let driver = outputs[0].0.clone().unwrap();
    assert_eq!(
        driver.get_instance_type().unwrap().get_type(),
        CellType::NOR2
    );
}

#[test]
fn test_aoi_map() {
    // NOR(AND(b1,b2),a) = AOI21(a,b1,b2)
    let nl = aoi21_netlist();
    let mut folder = Folder::<Cell>::new(101);
    folder.insert(AoiMap);

    let before = nl.len();
    assert_eq!(before, 5);

    let res = folder.run(&nl);
    assert!(res.is_ok());

    assert_eq!(nl.len(), before - 1);

    let outputs = nl.outputs();
    assert_eq!(outputs.len(), 1);

    let driver = outputs[0].0.clone().unwrap();
    assert_eq!(
        driver.get_instance_type().unwrap().get_type(),
        CellType::AOI21
    );
}

#[test]
fn test_oai_map() {
    // NAND(OR(b1,b2),a) = OAI21(a,b1,b2)
    let nl = oai21_netlist();
    let mut folder = Folder::<Cell>::new(101);
    folder.insert(OaiMap);

    let before = nl.len();
    assert_eq!(before, 5);

    let res = folder.run(&nl);
    assert!(res.is_ok());

    assert_eq!(nl.len(), before - 1);

    let outputs = nl.outputs();
    assert_eq!(outputs.len(), 1);

    let driver = outputs[0].0.clone().unwrap();
    assert_eq!(
        driver.get_instance_type().unwrap().get_type(),
        CellType::OAI21
    );
}

#[test]
fn test_aoi_map22() {
    // NOR(AND(a1,a2),AND(b1,b2)) = AOI22(a1,a2,b1,b2)
    let nl = aoi22_netlist();
    let mut folder = Folder::<Cell>::new(101);
    folder.insert(AoiMap22);

    let before = nl.len();
    assert_eq!(before, 7);

    let res = folder.run(&nl);
    assert!(res.is_ok());

    assert_eq!(nl.len(), before - 2);

    let outputs = nl.outputs();
    assert_eq!(outputs.len(), 1);

    let driver = outputs[0].0.clone().unwrap();
    assert_eq!(
        driver.get_instance_type().unwrap().get_type(),
        CellType::AOI22
    );
}

#[test]
fn test_oai_map22() {
    // NAND(OR(a1,a2),OR(b1,b2)) = OAI22(a1,a2,b1,b2)
    let nl = oai22_netlist();
    let mut folder = Folder::<Cell>::new(101);
    folder.insert(OaiMap22);

    let before = nl.len();
    assert_eq!(before, 7);

    let res = folder.run(&nl);
    assert!(res.is_ok());

    assert_eq!(nl.len(), before - 2);

    let outputs = nl.outputs();
    assert_eq!(outputs.len(), 1);

    let driver = outputs[0].0.clone().unwrap();
    assert_eq!(
        driver.get_instance_type().unwrap().get_type(),
        CellType::OAI22
    );
}

#[test]
fn test_mux_same_input() {
    // MUX(s,a,a) = a
    let nl = mux_same_input_netlist();
    let mut folder = Folder::<Cell>::new(101);
    folder.insert(MuxSameInput);

    let before = nl.len();
    assert_eq!(before, 3);

    let res = folder.run(&nl);
    assert!(res.is_ok());

    assert_eq!(nl.len(), before - 1);

    let outputs = nl.outputs();
    assert_eq!(outputs.len(), 1);
    assert!(outputs[0].0.is_an_input());
}

#[test]
fn test_mux_const_select() {
    // MUX(1,a,b) = a
    let nl = mux_const_select_netlist();
    let mut folder = Folder::<Cell>::new(101);
    folder.insert(MuxConstSelect);

    let before = nl.len();
    assert_eq!(before, 4);

    let res = folder.run(&nl);
    assert!(res.is_ok());

    assert_eq!(nl.len(), before - 2);

    let outputs = nl.outputs();
    assert_eq!(outputs.len(), 1);
    assert!(outputs[0].0.is_an_input());

    let inputs: Vec<_> = nl.inputs().collect();
    assert_eq!(inputs.len(), 2);
}