use super::{ContextInfo, DocumentMessage, ImageMessage, LocationMessage, VideoMessage};

#[derive(Clone, PartialEq, prost::Message)]
pub struct ButtonsMessage {
    #[prost(string, optional, tag = "1")]
    pub text: Option<String>,
    #[prost(message, optional, tag = "2")]
    pub document_message: Option<DocumentMessage>,
    #[prost(message, optional, tag = "3")]
    pub image_message: Option<ImageMessage>,
    #[prost(message, optional, tag = "4")]
    pub video_message: Option<VideoMessage>,
    #[prost(message, optional, tag = "5")]
    pub location_message: Option<LocationMessage>,
    #[prost(string, optional, tag = "6")]
    pub content_text: Option<String>,
    #[prost(string, optional, tag = "7")]
    pub footer_text: Option<String>,
    #[prost(message, optional, tag = "8")]
    pub context_info: Option<ContextInfo>,
    #[prost(message, repeated, tag = "9")]
    pub buttons: Vec<Button>,
    #[prost(int32, optional, tag = "10")]
    pub header_type: Option<i32>,
}

impl ButtonsMessage {
    pub const HEADER_EMPTY: i32 = 1;
    pub const HEADER_TEXT: i32 = 2;
    pub const HEADER_DOCUMENT: i32 = 3;
    pub const HEADER_IMAGE: i32 = 4;
    pub const HEADER_VIDEO: i32 = 5;
    pub const HEADER_LOCATION: i32 = 6;
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct Button {
    #[prost(string, optional, tag = "1")]
    pub button_id: Option<String>,
    #[prost(message, optional, tag = "2")]
    pub button_text: Option<ButtonText>,
    #[prost(int32, optional, tag = "3")]
    pub r#type: Option<i32>,
    #[prost(message, optional, tag = "4")]
    pub native_flow_info: Option<NativeFlowInfo>,
}

impl Button {
    pub const RESPONSE: i32 = 1;
    pub const NATIVE_FLOW: i32 = 2;
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct ButtonText {
    #[prost(string, optional, tag = "1")]
    pub display_text: Option<String>,
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct NativeFlowInfo {
    #[prost(string, optional, tag = "1")]
    pub name: Option<String>,
    #[prost(string, optional, tag = "2")]
    pub params_json: Option<String>,
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct ButtonsResponseMessage {
    #[prost(string, optional, tag = "1")]
    pub selected_button_id: Option<String>,
    #[prost(string, optional, tag = "2")]
    pub selected_display_text: Option<String>,
    #[prost(message, optional, tag = "3")]
    pub context_info: Option<ContextInfo>,
    #[prost(int32, optional, tag = "4")]
    pub r#type: Option<i32>,
}

impl ButtonsResponseMessage {
    pub const DISPLAY_TEXT: i32 = 1;
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct ListMessage {
    #[prost(string, optional, tag = "1")]
    pub title: Option<String>,
    #[prost(string, optional, tag = "2")]
    pub description: Option<String>,
    #[prost(string, optional, tag = "3")]
    pub button_text: Option<String>,
    #[prost(int32, optional, tag = "4")]
    pub list_type: Option<i32>,
    #[prost(message, repeated, tag = "5")]
    pub sections: Vec<ListSection>,
    #[prost(string, optional, tag = "7")]
    pub footer_text: Option<String>,
    #[prost(message, optional, tag = "8")]
    pub context_info: Option<ContextInfo>,
}

impl ListMessage {
    pub const SINGLE_SELECT: i32 = 1;
    pub const PRODUCT_LIST: i32 = 2;
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct ListSection {
    #[prost(string, optional, tag = "1")]
    pub title: Option<String>,
    #[prost(message, repeated, tag = "2")]
    pub rows: Vec<ListRow>,
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct ListRow {
    #[prost(string, optional, tag = "1")]
    pub title: Option<String>,
    #[prost(string, optional, tag = "2")]
    pub description: Option<String>,
    #[prost(string, optional, tag = "3")]
    pub row_id: Option<String>,
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct ListResponseMessage {
    #[prost(string, optional, tag = "1")]
    pub title: Option<String>,
    #[prost(int32, optional, tag = "2")]
    pub list_type: Option<i32>,
    #[prost(message, optional, tag = "3")]
    pub single_select_reply: Option<SingleSelectReply>,
    #[prost(message, optional, tag = "4")]
    pub context_info: Option<ContextInfo>,
    #[prost(string, optional, tag = "5")]
    pub description: Option<String>,
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct SingleSelectReply {
    #[prost(string, optional, tag = "1")]
    pub selected_row_id: Option<String>,
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct InteractiveMessage {
    #[prost(message, optional, tag = "1")]
    pub header: Option<InteractiveHeader>,
    #[prost(message, optional, tag = "2")]
    pub body: Option<InteractiveBody>,
    #[prost(message, optional, tag = "3")]
    pub footer: Option<InteractiveFooter>,
    #[prost(message, optional, tag = "6")]
    pub native_flow_message: Option<NativeFlowMessage>,
    #[prost(message, optional, tag = "7")]
    pub carousel_message: Option<CarouselMessage>,
    #[prost(message, optional, tag = "15")]
    pub context_info: Option<ContextInfo>,
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct InteractiveHeader {
    #[prost(string, optional, tag = "1")]
    pub title: Option<String>,
    #[prost(string, optional, tag = "2")]
    pub subtitle: Option<String>,
    #[prost(message, optional, tag = "3")]
    pub document_message: Option<DocumentMessage>,
    #[prost(message, optional, tag = "4")]
    pub image_message: Option<ImageMessage>,
    #[prost(bool, optional, tag = "5")]
    pub has_media_attachment: Option<bool>,
    #[prost(bytes = "vec", optional, tag = "6")]
    pub jpeg_thumbnail: Option<Vec<u8>>,
    #[prost(message, optional, tag = "7")]
    pub video_message: Option<VideoMessage>,
    #[prost(message, optional, tag = "8")]
    pub location_message: Option<LocationMessage>,
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct InteractiveBody {
    #[prost(string, optional, tag = "1")]
    pub text: Option<String>,
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct InteractiveFooter {
    #[prost(string, optional, tag = "1")]
    pub text: Option<String>,
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct NativeFlowMessage {
    #[prost(message, repeated, tag = "1")]
    pub buttons: Vec<NativeFlowButton>,
    #[prost(string, optional, tag = "2")]
    pub message_params_json: Option<String>,
    #[prost(int32, optional, tag = "3")]
    pub message_version: Option<i32>,
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct NativeFlowButton {
    #[prost(string, optional, tag = "1")]
    pub name: Option<String>,
    #[prost(string, optional, tag = "2")]
    pub button_params_json: Option<String>,
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct CarouselMessage {
    #[prost(message, repeated, tag = "1")]
    pub cards: Vec<InteractiveMessage>,
    #[prost(int32, optional, tag = "2")]
    pub message_version: Option<i32>,
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct InteractiveResponseMessage {
    #[prost(message, optional, tag = "1")]
    pub body: Option<InteractiveResponseBody>,
    #[prost(message, optional, tag = "2")]
    pub native_flow_response_message: Option<NativeFlowResponseMessage>,
    #[prost(message, optional, tag = "15")]
    pub context_info: Option<ContextInfo>,
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct InteractiveResponseBody {
    #[prost(string, optional, tag = "1")]
    pub text: Option<String>,
    #[prost(int32, optional, tag = "2")]
    pub format: Option<i32>,
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct NativeFlowResponseMessage {
    #[prost(string, optional, tag = "1")]
    pub name: Option<String>,
    #[prost(string, optional, tag = "2")]
    pub params_json: Option<String>,
    #[prost(int32, optional, tag = "3")]
    pub version: Option<i32>,
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct TemplateMessage {
    #[prost(message, optional, tag = "2")]
    pub hydrated_four_row_template: Option<HydratedFourRowTemplate>,
    #[prost(message, optional, tag = "3")]
    pub context_info: Option<ContextInfo>,
    #[prost(message, optional, tag = "4")]
    pub hydrated_template: Option<HydratedFourRowTemplate>,
    #[prost(string, optional, tag = "9")]
    pub template_id: Option<String>,
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct HydratedFourRowTemplate {
    #[prost(message, optional, tag = "1")]
    pub document_message: Option<DocumentMessage>,
    #[prost(string, optional, tag = "2")]
    pub hydrated_title_text: Option<String>,
    #[prost(message, optional, tag = "3")]
    pub image_message: Option<ImageMessage>,
    #[prost(message, optional, tag = "4")]
    pub video_message: Option<VideoMessage>,
    #[prost(message, optional, tag = "5")]
    pub location_message: Option<LocationMessage>,
    #[prost(string, optional, tag = "6")]
    pub hydrated_content_text: Option<String>,
    #[prost(string, optional, tag = "7")]
    pub hydrated_footer_text: Option<String>,
    #[prost(message, repeated, tag = "8")]
    pub hydrated_buttons: Vec<HydratedTemplateButton>,
    #[prost(string, optional, tag = "9")]
    pub template_id: Option<String>,
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct HydratedTemplateButton {
    #[prost(message, optional, tag = "1")]
    pub quick_reply_button: Option<HydratedQuickReplyButton>,
    #[prost(message, optional, tag = "2")]
    pub url_button: Option<HydratedUrlButton>,
    #[prost(message, optional, tag = "3")]
    pub call_button: Option<HydratedCallButton>,
    #[prost(uint32, optional, tag = "4")]
    pub index: Option<u32>,
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct HydratedQuickReplyButton {
    #[prost(string, optional, tag = "1")]
    pub display_text: Option<String>,
    #[prost(string, optional, tag = "2")]
    pub id: Option<String>,
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct HydratedUrlButton {
    #[prost(string, optional, tag = "1")]
    pub display_text: Option<String>,
    #[prost(string, optional, tag = "2")]
    pub url: Option<String>,
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct HydratedCallButton {
    #[prost(string, optional, tag = "1")]
    pub display_text: Option<String>,
    #[prost(string, optional, tag = "2")]
    pub phone_number: Option<String>,
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct TemplateButtonReplyMessage {
    #[prost(string, optional, tag = "1")]
    pub selected_id: Option<String>,
    #[prost(string, optional, tag = "2")]
    pub selected_display_text: Option<String>,
    #[prost(message, optional, tag = "3")]
    pub context_info: Option<ContextInfo>,
    #[prost(uint32, optional, tag = "4")]
    pub selected_index: Option<u32>,
}
