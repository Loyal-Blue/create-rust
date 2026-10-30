fn render(game: &mut crate::game::Game<'_>, camera: glam::DVec3, alpha: f64, out: &mut Vec<horizon_client::mesher::Vertex>) {
    crate::game::create_runtime::draw_rotation(game,camera,alpha,out);
}
