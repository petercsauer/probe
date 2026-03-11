use std::collections::HashMap;
use std::io;
use std::time::{Duration, Instant};

use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers};
use crossterm::terminal::{self, EnterAlternateScreen, LeaveAlternateScreen};
use crossterm::{execute, event::EnableMouseCapture, event::DisableMouseCapture};
use ratatui::backend::CrosstermBackend;
use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Widget};
use unicode_width::UnicodeWidthStr;
use ratatui::Terminal;
use tui_input::backend::crossterm::EventHandler;
use tui_input::Input;

use crate::config::Config;
use crate::event_store::EventStore;
use crate::live::{AppEvent, CaptureState, LiveDataSource};
use crate::panes::decode_tree::DecodeTreePane;
use crate::panes::event_list::EventListPane;
use crate::panes::hex_dump::HexDumpPane;
use crate::panes::timeline::TimelinePane;
use crate::panes::{Action, PaneComponent};
use crate::ring_buffer::RingBuffer;
use crate::theme::{Theme, ThemeConfig};

use prb_capture::CaptureStats;
use prb_query::Filter;
use prb_schema::SchemaRegistry;
use prb_core::{Payload, METADATA_KEY_GRPC_METHOD};
use prb_decode::{decode_with_schema, decode_wire_format, WireMessage, WireValue, LenValue};
use serde_json::{json, Value as JsonValue};
/// Convert a wire-format decoded message to JSON.
fn wire_message_to_json(wire_msg: &WireMessage) -> JsonValue {
    let mut fields = serde_json::Map::new();

    for field in &wire_msg.fields {
        let key = format!("field_{}", field.field_number);
        let value = match &field.value {
            WireValue::Varint(v) => {
                json!({
                    "wire_type": "varint",
                    "unsigned": v.unsigned,
                    "signed_zigzag": v.signed_zigzag,
                    "as_bool": v.as_bool,
                })
            }
            WireValue::Fixed64(v) => {
                json!({
                    "wire_type": "fixed64",
                    "as_u64": v.as_u64,
                    "as_i64": v.as_i64,
                    "as_f64": v.as_f64,
                })
            }
            WireValue::Fixed32(v) => {
                json!({
                    "wire_type": "fixed32",
                    "as_u32": v.as_u32,
                    "as_i32": v.as_i32,
                    "as_f32": v.as_f32,
                })
            }
            WireValue::LengthDelimited(len_val) => match len_val {
                LenValue::SubMessage(sub_msg) => {
                    json!({
                        "wire_type": "length_delimited",
                        "sub_message": wire_message_to_json(sub_msg),
                    })
                }
                LenValue::String(s) => {
                    json!({
                        "wire_type": "length_delimited",
                        "string": s,
                    })
                }
                LenValue::Bytes(b) => {
                    json!({
                        "wire_type": "length_delimited",
                        "bytes": format!("{} bytes", b.len()),
                    })
                }
            },
        };
        fields.insert(key, value);
    }

    JsonValue::Object(fields)
}
