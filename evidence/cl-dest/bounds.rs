//! CL diagnostic-only sealed-state frontier, not an admission bypass API.
use super::*;

#[test]
fn all_destination_slots_round_trip_without_truncation() {
    let mut ops = Vec::new();
    let mut lower = Lower {
        ops: &mut ops,
        next: 0,
    };
    let mut state = State {
        locals: [0; MAX_LOCALS],
        pc: 0,
        index: 0,
        chunk_left: 0,
        remaining: 0,
        row: None,
    };
    let mut temps = [0; MAX_NODES];
    assert!(MAX_LOCALS <= usize::from(u8::MAX) + 1);
    assert!(MAX_NODES <= usize::from(u16::MAX) + 1);
    for slot in 0..MAX_NODES {
        let d = lower.temp();
        let Dest::Temp(i) = d else {
            panic!("temporary")
        };
        assert_eq!(usize::from(i), slot);
        write(d, -(slot as i64) - 1, &mut state, &mut temps);
        assert_eq!(
            read(Operand::Temp(slot), &state, &temps),
            -(slot as i64) - 1
        );
    }
    for slot in 0..MAX_LOCALS {
        write(
            Dest::Local(u8::try_from(slot).unwrap()),
            slot as i64,
            &mut state,
            &mut temps,
        );
        assert_eq!(read(Operand::Local(slot), &state, &temps), slot as i64);
    }
    println!(
        "CL_BOUNDS locals={} temps={} highest_temp={}",
        MAX_LOCALS,
        MAX_NODES,
        MAX_NODES - 1
    );
}

#[test]
#[should_panic]
fn allocator_retains_existing_max_nodes_assertion() {
    let mut ops = Vec::new();
    Lower {
        ops: &mut ops,
        next: MAX_NODES,
    }
    .temp();
}

#[test]
#[should_panic]
fn invalid_sealed_temp_still_hits_checked_index() {
    let mut state = State {
        locals: [0; MAX_LOCALS],
        pc: 0,
        index: 0,
        chunk_left: 0,
        remaining: 0,
        row: None,
    };
    write(
        Dest::Temp(u16::try_from(MAX_NODES).unwrap()),
        1,
        &mut state,
        &mut [0; MAX_NODES],
    );
}

#[test]
#[should_panic]
fn invalid_sealed_local_still_hits_checked_index() {
    let mut state = State {
        locals: [0; MAX_LOCALS],
        pc: 0,
        index: 0,
        chunk_left: 0,
        remaining: 0,
        row: None,
    };
    write(
        Dest::Local(u8::try_from(MAX_LOCALS).unwrap()),
        1,
        &mut state,
        &mut [0; MAX_NODES],
    );
}
