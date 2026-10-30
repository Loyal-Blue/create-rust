// Canonical host-stack decoding for authored recipe outputs and client previews.
fn decode_portable_item(stack: &machines::Stack) -> Option<ItemStack> {
    if !recipes::valid_id(&stack.id)
        || stack.count == 0
        || stack.count > 128
        || stack.components.len() > 65536
    {
        return None;
    }
    let id = serde_json::to_string(&stack.id).ok()?;
    let components = if stack.components.is_empty() {
        "{}"
    } else {
        &stack.components
    };
    let text = format!("{{id:{id},count:{},components:{components}}}", stack.count);
    horizon_client::item_data::parse_stack(&text).ok().flatten()
}
