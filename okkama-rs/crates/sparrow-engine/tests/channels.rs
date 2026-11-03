use sparrow_engine::{Executable, Mode};
#[test]
fn buffered_and_rendezvous_channels_preserve_fifo_cancel_and_close_contracts() {
    let program = okkama_sparrow::assemble(include_str!("fixtures/async-channel.asm")).unwrap();
    for mode in [Mode::Cold, Mode::Jit] {
        assert_eq!(Executable::new(&program, mode).unwrap().run().unwrap(), 42);
    }
}

#[test]
fn sender_and_receiver_cannot_disagree_about_the_transported_type() {
    let mut program = okkama_sparrow::assemble(include_str!("fixtures/async-channel.asm")).unwrap();
    let boolean = program
        .types
        .iter()
        .position(|t| matches!(t, okkama_sparrow::Type::Bool))
        .unwrap();
    let receive = program
        .functions
        .iter_mut()
        .find(|f| {
            f.intrinsic
                .as_ref()
                .is_some_and(|i| i.name == "okkama.channel.Receiving.item")
        })
        .unwrap();
    receive.result = Some(okkama_sparrow::TypeRef(boolean as u32));
    for mode in [Mode::Cold, Mode::Jit] {
        assert!(Executable::new(&program, mode).is_err());
    }
}

#[test]
fn buffered_payload_retention_does_not_transfer_lexical_destroy_ownership() {
    let program =
        okkama_sparrow::assemble(include_str!("fixtures/async-channel-resources.asm")).unwrap();
    for mode in [Mode::Cold, Mode::Jit] {
        assert_eq!(Executable::new(&program, mode).unwrap().run().unwrap(), 42);
    }
}
