fn select_page(screen: &crate::creative::Creative, tab: usize) {
    if let Some(tab)=screen.tabs.get(tab) {
        if tab.id=="create:main" {crate::mod_content::create_preview_support::PAGE.store(1,std::sync::atomic::Ordering::Relaxed)}
        else if !tab.search&&!tab.inventory {crate::mod_content::create_preview_support::PAGE.store(0,std::sync::atomic::Ordering::Relaxed)}
    }
}
