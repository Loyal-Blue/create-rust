fn name(block:&str)->core::ops::ControlFlow<Option<&'static str>>{if block.starts_with("create:"){core::ops::ControlFlow::Break(Some("container.chest"))}else{core::ops::ControlFlow::Continue(())}}
