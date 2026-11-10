use serde_json::{json, Value};

use crate::binary::Node;
use crate::proto::{
    Button, ButtonText, ButtonsMessage, DocumentMessage, FutureProofMessage, HydratedCallButton,
    HydratedFourRowTemplate, HydratedQuickReplyButton, HydratedTemplateButton, HydratedUrlButton, ImageMessage,
    InteractiveBody, InteractiveFooter, InteractiveHeader, InteractiveMessage, ListMessage, ListRow, ListSection,
    Message, MessageContextInfo, NativeFlowButton, NativeFlowMessage, TemplateMessage, VideoMessage,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Row {
    pub id: String,
    pub title: String,
    pub description: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Section {
    pub title: String,
    pub rows: Vec<Row>,
}

impl Section {
    pub fn new(title: impl Into<String>) -> Self {
        Section { title: title.into(), rows: Vec::new() }
    }

    pub fn row(mut self, id: impl Into<String>, title: impl Into<String>) -> Self {
        self.rows.push(Row { id: id.into(), title: title.into(), description: None });
        self
    }

    pub fn described_row(
        mut self,
        id: impl Into<String>,
        title: impl Into<String>,
        description: impl Into<String>,
    ) -> Self {
        self.rows.push(Row { id: id.into(), title: title.into(), description: Some(description.into()) });
        self
    }

    fn to_proto(&self) -> ListSection {
        ListSection {
            title: Some(self.title.clone()),
            rows: self
                .rows
                .iter()
                .map(|row| ListRow {
                    title: Some(row.title.clone()),
                    description: row.description.clone(),
                    row_id: Some(row.id.clone()),
                })
                .collect(),
        }
    }

    fn to_json(&self) -> Value {
        json!({
            "title": self.title,
            "rows": self.rows.iter().map(|row| json!({
                "header": "",
                "title": row.title,
                "description": row.description.clone().unwrap_or_default(),
                "id": row.id,
            })).collect::<Vec<_>>(),
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NativeButton {
    QuickReply { id: String, text: String },
    Url { text: String, url: String },
    Copy { text: String, code: String },
    Call { text: String, phone: String },
    Select { title: String, sections: Vec<Section> },
}

impl NativeButton {
    fn to_proto(&self) -> NativeFlowButton {
        let (name, params) = match self {
            NativeButton::QuickReply { id, text } => ("quick_reply", json!({ "display_text": text, "id": id })),
            NativeButton::Url { text, url } => {
                ("cta_url", json!({ "display_text": text, "url": url, "merchant_url": url }))
            }
            NativeButton::Copy { text, code } => {
                ("cta_copy", json!({ "display_text": text, "copy_code": code, "id": code }))
            }
            NativeButton::Call { text, phone } => {
                ("cta_call", json!({ "display_text": text, "phone_number": phone, "id": phone }))
            }
            NativeButton::Select { title, sections } => (
                "single_select",
                json!({ "title": title, "sections": sections.iter().map(Section::to_json).collect::<Vec<_>>() }),
            ),
        };
        NativeFlowButton { name: Some(name.into()), button_params_json: Some(params.to_string()) }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TemplateButton {
    QuickReply { id: String, text: String },
    Url { text: String, url: String },
    Call { text: String, phone: String },
}

#[derive(Clone, Debug, Default)]
pub struct Interactive {
    body: String,
    footer: Option<String>,
    header: InteractiveHeader,
    buttons: Vec<NativeButton>,
}

impl Interactive {
    pub fn new(body: impl Into<String>) -> Self {
        Interactive { body: body.into(), ..Default::default() }
    }

    pub fn footer(mut self, text: impl Into<String>) -> Self {
        self.footer = Some(text.into());
        self
    }

    pub fn title(mut self, text: impl Into<String>) -> Self {
        self.header.title = Some(text.into());
        self
    }

    pub fn subtitle(mut self, text: impl Into<String>) -> Self {
        self.header.subtitle = Some(text.into());
        self
    }

    pub fn image(mut self, image: ImageMessage) -> Self {
        self.header.image_message = Some(image);
        self.header.has_media_attachment = Some(true);
        self
    }

    pub fn video(mut self, video: VideoMessage) -> Self {
        self.header.video_message = Some(video);
        self.header.has_media_attachment = Some(true);
        self
    }

    pub fn document(mut self, document: DocumentMessage) -> Self {
        self.header.document_message = Some(document);
        self.header.has_media_attachment = Some(true);
        self
    }

    pub fn button(mut self, button: NativeButton) -> Self {
        self.buttons.push(button);
        self
    }

    pub fn quick_reply(self, id: impl Into<String>, text: impl Into<String>) -> Self {
        self.button(NativeButton::QuickReply { id: id.into(), text: text.into() })
    }

    pub fn url(self, text: impl Into<String>, url: impl Into<String>) -> Self {
        self.button(NativeButton::Url { text: text.into(), url: url.into() })
    }

    pub fn copy(self, text: impl Into<String>, code: impl Into<String>) -> Self {
        self.button(NativeButton::Copy { text: text.into(), code: code.into() })
    }

    pub fn call(self, text: impl Into<String>, phone: impl Into<String>) -> Self {
        self.button(NativeButton::Call { text: text.into(), phone: phone.into() })
    }

    pub fn select(self, title: impl Into<String>, sections: Vec<Section>) -> Self {
        self.button(NativeButton::Select { title: title.into(), sections })
    }

    pub fn build(self) -> Message {
        let has_header = self.header != InteractiveHeader::default();
        let interactive = InteractiveMessage {
            header: has_header.then_some(self.header),
            body: Some(InteractiveBody { text: Some(self.body) }),
            footer: self.footer.map(|text| InteractiveFooter { text: Some(text) }),
            native_flow_message: Some(NativeFlowMessage {
                buttons: self.buttons.iter().map(NativeButton::to_proto).collect(),
                message_params_json: Some("{}".into()),
                message_version: Some(1),
            }),
            carousel_message: None,
            context_info: None,
        };
        let inner = Message {
            message_context_info: Some(Box::new(MessageContextInfo {
                device_list_metadata_version: Some(2),
                ..Default::default()
            })),
            interactive_message: Some(Box::new(interactive)),
            ..Default::default()
        };
        Message {
            view_once_message: Some(Box::new(FutureProofMessage { message: Some(Box::new(inner)) })),
            ..Default::default()
        }
    }
}

impl From<Interactive> for Message {
    fn from(value: Interactive) -> Self {
        value.build()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReplySource {
    Buttons,
    List,
    Template,
    NativeFlow,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InteractiveReply {
    pub id: String,
    pub text: Option<String>,
    pub source: ReplySource,
    pub flow: Option<String>,
    pub params: Option<Value>,
}

impl Message {
    pub fn buttons<I, A, B>(text: impl Into<String>, footer: Option<&str>, buttons: I) -> Self
    where
        I: IntoIterator<Item = (A, B)>,
        A: Into<String>,
        B: Into<String>,
    {
        let mut builder = Interactive::new(text);
        if let Some(footer) = footer {
            builder = builder.footer(footer);
        }
        buttons.into_iter().fold(builder, |builder, (id, label)| builder.quick_reply(id, label)).build()
    }

    pub fn list(
        title: impl Into<String>,
        text: impl Into<String>,
        button_text: impl Into<String>,
        footer: Option<&str>,
        sections: &[Section],
    ) -> Self {
        let mut builder = Interactive::new(text).title(title);
        if let Some(footer) = footer {
            builder = builder.footer(footer);
        }
        builder.select(button_text, sections.to_vec()).build()
    }

    pub fn legacy_buttons<I, A, B>(text: impl Into<String>, footer: Option<&str>, buttons: I) -> Self
    where
        I: IntoIterator<Item = (A, B)>,
        A: Into<String>,
        B: Into<String>,
    {
        let buttons = buttons
            .into_iter()
            .map(|(id, label)| Button {
                button_id: Some(id.into()),
                button_text: Some(ButtonText { display_text: Some(label.into()) }),
                r#type: Some(Button::RESPONSE),
                native_flow_info: None,
            })
            .collect();
        Message {
            buttons_message: Some(Box::new(ButtonsMessage {
                content_text: Some(text.into()),
                footer_text: footer.map(str::to_owned),
                buttons,
                header_type: Some(ButtonsMessage::HEADER_EMPTY),
                ..Default::default()
            })),
            ..Default::default()
        }
    }

    pub fn legacy_list(
        title: impl Into<String>,
        text: impl Into<String>,
        button_text: impl Into<String>,
        footer: Option<&str>,
        sections: &[Section],
    ) -> Self {
        Message {
            list_message: Some(Box::new(ListMessage {
                title: Some(title.into()),
                description: Some(text.into()),
                button_text: Some(button_text.into()),
                list_type: Some(ListMessage::SINGLE_SELECT),
                sections: sections.iter().map(Section::to_proto).collect(),
                footer_text: footer.map(str::to_owned),
                context_info: None,
            })),
            ..Default::default()
        }
    }

    pub fn template(text: impl Into<String>, footer: Option<&str>, buttons: &[TemplateButton]) -> Self {
        let hydrated_buttons = buttons
            .iter()
            .enumerate()
            .map(|(index, button)| {
                let mut hydrated = HydratedTemplateButton { index: Some(index as u32 + 1), ..Default::default() };
                match button {
                    TemplateButton::QuickReply { id, text } => {
                        hydrated.quick_reply_button =
                            Some(HydratedQuickReplyButton { display_text: Some(text.clone()), id: Some(id.clone()) })
                    }
                    TemplateButton::Url { text, url } => {
                        hydrated.url_button =
                            Some(HydratedUrlButton { display_text: Some(text.clone()), url: Some(url.clone()) })
                    }
                    TemplateButton::Call { text, phone } => {
                        hydrated.call_button = Some(HydratedCallButton {
                            display_text: Some(text.clone()),
                            phone_number: Some(phone.clone()),
                        })
                    }
                }
                hydrated
            })
            .collect();
        let template = HydratedFourRowTemplate {
            hydrated_content_text: Some(text.into()),
            hydrated_footer_text: footer.map(str::to_owned),
            hydrated_buttons,
            ..Default::default()
        };
        Message {
            template_message: Some(Box::new(TemplateMessage {
                hydrated_four_row_template: Some(template.clone()),
                hydrated_template: Some(template),
                ..Default::default()
            })),
            ..Default::default()
        }
    }

    pub fn interactive_reply(&self) -> Option<InteractiveReply> {
        let m = self.unwrapped();
        if let Some(reply) = &m.buttons_response_message {
            return Some(InteractiveReply {
                id: reply.selected_button_id.clone()?,
                text: reply.selected_display_text.clone(),
                source: ReplySource::Buttons,
                flow: None,
                params: None,
            });
        }
        if let Some(reply) = &m.list_response_message {
            return Some(InteractiveReply {
                id: reply.single_select_reply.as_ref()?.selected_row_id.clone()?,
                text: reply.title.clone(),
                source: ReplySource::List,
                flow: None,
                params: None,
            });
        }
        if let Some(reply) = &m.template_button_reply_message {
            return Some(InteractiveReply {
                id: reply.selected_id.clone()?,
                text: reply.selected_display_text.clone(),
                source: ReplySource::Template,
                flow: None,
                params: None,
            });
        }
        let reply = m.interactive_response_message.as_ref()?;
        let flow = reply.native_flow_response_message.as_ref()?;
        let params: Option<Value> = flow.params_json.as_deref().and_then(|raw| serde_json::from_str(raw).ok());
        let id = params.as_ref().and_then(|p| p["id"].as_str()).map(str::to_owned).or_else(|| flow.name.clone())?;
        Some(InteractiveReply {
            id,
            text: reply.body.as_ref().and_then(|b| b.text.clone()),
            source: ReplySource::NativeFlow,
            flow: flow.name.clone(),
            params,
        })
    }
}

pub(crate) fn biz_node(message: &Message) -> Option<Node> {
    let m = message.unwrapped();
    let child = if m.interactive_message.is_some() {
        Node::new("interactive")
            .attr("type", "native_flow")
            .attr("v", "1")
            .child_node(Node::new("native_flow").attr("v", "9").attr("name", "mixed"))
    } else if m.buttons_message.is_some() {
        Node::new("buttons")
    } else if let Some(list) = &m.list_message {
        let kind = if list.list_type == Some(ListMessage::PRODUCT_LIST) { "product_list" } else { "single_select" };
        Node::new("list").attr("v", "2").attr("type", kind)
    } else if m.buttons_response_message.is_some() {
        Node::new("buttons_response")
    } else if m.list_response_message.is_some() {
        Node::new("list_response")
    } else if m.interactive_response_message.is_some() {
        Node::new("interactive_response")
    } else {
        return None;
    };
    Some(Node::new("biz").child_node(child))
}

#[cfg(test)]
mod tests {
    use prost::Message as _;

    use super::*;
    use crate::proto::{
        ButtonsResponseMessage, InteractiveResponseBody, InteractiveResponseMessage, ListResponseMessage,
        NativeFlowResponseMessage, SingleSelectReply,
    };

    #[test]
    fn native_flow() {
        let message = Interactive::new("¿Qué quieres hacer?")
            .title("Soblend")
            .footer("SoblendWA")
            .quick_reply("ping", "Ping")
            .url("Sitio", "https://soblend.dev")
            .copy("Copiar código", "SOBLEND-2026")
            .select("Opciones", vec![Section::new("Comandos").described_row("menu", "Menú", "Todos los comandos")])
            .build();
        let decoded = Message::decode(message.encode_to_vec().as_slice()).unwrap();
        let inner = decoded.unwrapped().interactive_message.as_ref().unwrap();
        let buttons = &inner.native_flow_message.as_ref().unwrap().buttons;
        assert_eq!(buttons.len(), 4);
        assert_eq!(buttons[0].name.as_deref(), Some("quick_reply"));
        let params: Value = serde_json::from_str(buttons[3].button_params_json.as_deref().unwrap()).unwrap();
        assert_eq!(params["sections"][0]["rows"][0]["id"], "menu");
        assert_eq!(decoded.get_text(), Some("¿Qué quieres hacer?"));
        let biz = biz_node(&decoded).unwrap();
        assert_eq!(biz.path(&["interactive", "native_flow"]).and_then(|n| n.attr_str("name")), Some("mixed"));
    }

    #[test]
    fn legacy_buttons_biz_tag() {
        let buttons = Message::legacy_buttons("Elige", Some("pie"), [("a", "Opción A"), ("b", "Opción B")]);
        assert_eq!(buttons.buttons_message.as_ref().unwrap().buttons.len(), 2);
        assert!(biz_node(&buttons).unwrap().child("buttons").is_some());

        let list =
            Message::legacy_list("Menú", "Elige", "Ver menú", None, &[Section::new("Tacos").row("pastor", "Pastor")]);
        let biz = biz_node(&list).unwrap();
        assert_eq!(biz.child("list").and_then(|n| n.attr_str("type")), Some("single_select"));

        let modern = Message::buttons("Elige", Some("pie"), [("a", "Opción A"), ("b", "Opción B")]);
        let flow = modern.unwrapped().interactive_message.as_ref().unwrap().native_flow_message.as_ref().unwrap();
        assert_eq!(flow.buttons.len(), 2);
        assert_eq!(flow.buttons[0].name.as_deref(), Some("quick_reply"));
        let menu = Message::list("Menú", "Elige", "Ver menú", None, &[Section::new("Tacos").row("pastor", "Pastor")]);
        let select =
            &menu.unwrapped().interactive_message.as_ref().unwrap().native_flow_message.as_ref().unwrap().buttons[0];
        assert_eq!(select.name.as_deref(), Some("single_select"));
        assert!(biz_node(&menu).unwrap().child("interactive").is_some());

        let template = Message::template(
            "Hola",
            None,
            &[TemplateButton::Url { text: "Web".into(), url: "https://soblend.dev".into() }],
        );
        let hydrated = template.template_message.as_ref().unwrap().hydrated_template.as_ref().unwrap();
        assert_eq!(hydrated.hydrated_buttons[0].index, Some(1));
        assert!(biz_node(&Message::text("hola")).is_none());
    }

    #[test]
    fn selected_reply() {
        let tapped = Message {
            buttons_response_message: Some(Box::new(ButtonsResponseMessage {
                selected_button_id: Some("a".into()),
                selected_display_text: Some("Opción A".into()),
                ..Default::default()
            })),
            ..Default::default()
        };
        let reply = tapped.interactive_reply().unwrap();
        assert_eq!((reply.id.as_str(), reply.source), ("a", ReplySource::Buttons));
        assert_eq!(tapped.get_text(), Some("Opción A"));

        let row = Message {
            list_response_message: Some(Box::new(ListResponseMessage {
                title: Some("Pastor".into()),
                single_select_reply: Some(SingleSelectReply { selected_row_id: Some("pastor".into()) }),
                ..Default::default()
            })),
            ..Default::default()
        };
        assert_eq!(row.interactive_reply().unwrap().id, "pastor");

        let flow = Message {
            interactive_response_message: Some(Box::new(InteractiveResponseMessage {
                body: Some(InteractiveResponseBody { text: Some("Ping".into()), format: None }),
                native_flow_response_message: Some(NativeFlowResponseMessage {
                    name: Some("quick_reply".into()),
                    params_json: Some(r#"{"id":"ping"}"#.into()),
                    version: Some(3),
                }),
                context_info: None,
            })),
            ..Default::default()
        };
        let reply = flow.interactive_reply().unwrap();
        assert_eq!((reply.id.as_str(), reply.flow.as_deref()), ("ping", Some("quick_reply")));
        assert_eq!(flow.get_text(), Some("Ping"));
    }
}
