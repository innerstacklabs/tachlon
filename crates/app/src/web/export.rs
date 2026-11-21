//! The session as an MS Project XML file for the page's Export… dialog (gs#703).

use super::App;
use wasm_bindgen::prelude::*;
use web_sys::js_sys::{Array, Object, Reflect, Uint8Array};

#[wasm_bindgen]
impl App {
    /// The session as an MS Project XML file, edits included: `{ bytes, left_out }`, the UTF-8
    /// bytes of the file as a `Uint8Array` and a `string[]` naming what the file cannot carry.
    /// Reads the session and changes nothing. Throws `JsError` when a date or duration cannot be
    /// written in the file's form.
    pub fn export_mspdi(&self) -> Result<JsValue, JsError> {
        self.exported(crate::export::export_mspdi)
    }

    /// The session as a P6 XER file, edits included: `{ bytes, left_out }` as `export_mspdi`
    /// returns it. Reads the session and changes nothing. Throws `JsError` when a date cannot be
    /// written in the file's form or a cell holds a character the file cannot carry.
    pub fn export_xer(&self) -> Result<JsValue, JsError> {
        self.exported(crate::export::export_xer)
    }

    /// What a Save as XER would leave out of this schedule, as a `string[]` from
    /// `export::xer_left_out`: the activity codes, baselines and deadlines it has, each with its
    /// task count, and nothing when it has none. Writes no file; reads the session and changes
    /// nothing.
    pub fn export_xer_left_out(&self) -> Array {
        crate::export::xer_left_out(&self.session)
            .iter()
            .map(|s| JsValue::from_str(s))
            .collect()
    }
}

impl App {
    /// Run `write` over the session and hand its bytes and `left_out` lines to the page.
    fn exported(
        &self,
        write: fn(
            &crate::Session,
            &crate::document::Extras<'_>,
        ) -> Result<import::Exported, import::ImportError>,
    ) -> Result<JsValue, JsError> {
        let source = self
            .source
            .as_ref()
            .map(|(format, bytes)| crate::document::Source {
                format: *format,
                bytes,
            });
        let exported = write(
            &self.session,
            &crate::document::Extras {
                name: "",
                notes: &self.notes,
                source,
            },
        )
        .map_err(|error| JsError::new(&error.to_string()))?;
        let left_out: Array = exported
            .left_out
            .iter()
            .map(|s| JsValue::from_str(s))
            .collect();
        let out = Object::new();
        let set = |key: &str, value: &JsValue| {
            Reflect::set(&out, &JsValue::from_str(key), value).map(|_| ())
        };
        set("bytes", &Uint8Array::from(exported.bytes.as_slice()))
            .and_then(|()| set("left_out", &left_out))
            .map_err(|_| JsError::new("could not build the export"))?;
        Ok(out.into())
    }
}
