fn position(tab: &crate::creative::Tab) -> core::ops::ControlFlow<(f32, f32)> {
    let page=crate::mod_content::create_preview_support::PAGE.load(std::sync::atomic::Ordering::Relaxed);
    let visible=crate::mod_content::create_preview_support::page_visible(&tab.id,tab.search,tab.inventory,page);
    if visible {core::ops::ControlFlow::Continue(())}else{core::ops::ControlFlow::Break((-1000000.0,-1000000.0))}
}
