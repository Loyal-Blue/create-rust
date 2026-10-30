fn remember(game: &crate::game::Game<'_>) {
    crate::game::create_runtime::remember_menu_target(game,game.remote.as_ref().and_then(|r|r.use_target));
}
