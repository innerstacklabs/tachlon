//! The host API's capability profile: what the host's database can take.

use super::App;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
impl App {
    /// Tell the chart what the host can take, as `{"gestures":{"barMove":false},"editable":{"dates":false}}` (`docs/WASM_API.md` lists
    /// the members; each is on unless `false`, and `{}` allows everything). A change a member turned off is refused with `"notAllowed"`.
    /// Throws `JsError`, changing nothing, for text that is not a profile. Not an edit; opening another schedule keeps it.
    pub fn set_capabilities(&mut self, json: &str) -> Result<(), JsError> {
        let set = self.session.set_capabilities(json);
        set.map_err(|error| JsError::new(&error))
    }
}
