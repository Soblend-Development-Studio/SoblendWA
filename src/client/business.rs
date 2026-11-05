use futures_util::future::try_join_all;

use super::Client;
use crate::binary::Node;
use crate::error::{Error, Result};
use crate::jid::Jid;

const MEDIA_ORIGIN: &str = "https://mmg.whatsapp.net";
const PRODUCT_IMAGE_PATH: &str = "/product/image";
const COVER_PHOTO_PATH: &str = "/pps/biz-cover-photo";
const PREVIEW_SIDE: &str = "100";
const MAX_REMOTE_IMAGE: usize = 8 * 1024 * 1024;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CatalogImage {
    Url(String),
    Data(Vec<u8>),
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ProductImage {
    pub requested: Option<String>,
    pub original: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Product {
    pub id: String,
    pub name: String,
    pub retailer_id: Option<String>,
    pub url: Option<String>,
    pub description: Option<String>,
    pub price: Option<u64>,
    pub currency: Option<String>,
    pub hidden: bool,
    pub images: Vec<ProductImage>,
    pub review_status: Option<String>,
    pub can_appeal: bool,
}

#[derive(Clone, Debug, Default)]
pub struct ProductDraft {
    pub name: Option<String>,
    pub description: Option<String>,
    pub retailer_id: Option<String>,
    pub price: Option<u64>,
    pub currency: Option<String>,
    pub images: Vec<CatalogImage>,
    pub hidden: Option<bool>,
    pub origin_country: Option<String>,
}

#[derive(Clone, Debug, Default)]
pub struct Catalog {
    pub products: Vec<Product>,
    pub next_cursor: Option<String>,
}

#[derive(Clone, Debug, Default)]
pub struct ProductCollection {
    pub id: String,
    pub name: String,
    pub products: Vec<Product>,
    pub status: Option<String>,
    pub can_appeal: bool,
}

#[derive(Clone, Debug, Default)]
pub struct OrderItem {
    pub id: String,
    pub name: String,
    pub image_url: Option<String>,
    pub price: Option<u64>,
    pub currency: Option<String>,
    pub quantity: u32,
}

#[derive(Clone, Debug, Default)]
pub struct Order {
    pub items: Vec<OrderItem>,
    pub total: Option<u64>,
    pub currency: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Weekday {
    Sun,
    Mon,
    Tue,
    Wed,
    Thu,
    Fri,
    Sat,
}

impl Weekday {
    fn as_str(self) -> &'static str {
        match self {
            Weekday::Sun => "sun",
            Weekday::Mon => "mon",
            Weekday::Tue => "tue",
            Weekday::Wed => "wed",
            Weekday::Thu => "thu",
            Weekday::Fri => "fri",
            Weekday::Sat => "sat",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OpeningHours {
    AllDay,
    ByAppointment,
    Between { open_minute: u16, close_minute: u16 },
}

#[derive(Clone, Debug, Default)]
pub struct BusinessHours {
    pub timezone: String,
    pub days: Vec<(Weekday, OpeningHours)>,
}

#[derive(Clone, Debug, Default)]
pub struct BusinessProfileUpdate {
    pub description: Option<String>,
    pub address: Option<String>,
    pub email: Option<String>,
    pub websites: Option<Vec<String>>,
    pub hours: Option<BusinessHours>,
}

fn text(node: &Node, tag: &str) -> Option<String> {
    node.child(tag).and_then(Node::content_string).filter(|t| !t.is_empty())
}

fn number<T: std::str::FromStr>(node: &Node, tag: &str) -> Option<T> {
    text(node, tag)?.trim().parse().ok()
}

fn leaf(tag: &'static str, value: impl Into<Vec<u8>>) -> Node {
    Node::new(tag).bytes(value)
}

fn preview_size() -> [Node; 2] {
    [leaf("width", PREVIEW_SIDE), leaf("height", PREVIEW_SIDE)]
}

fn review(node: &Node) -> (Option<String>, bool) {
    match node.child("status_info") {
        Some(info) => (text(info, "status"), text(info, "can_appeal").as_deref() == Some("true")),
        None => (None, false),
    }
}

pub(crate) fn parse_product(node: &Node) -> Product {
    let (review_status, can_appeal) = review(node);
    Product {
        id: text(node, "id").unwrap_or_default(),
        name: text(node, "name").unwrap_or_default(),
        retailer_id: text(node, "retailer_id"),
        url: text(node, "url"),
        description: text(node, "description"),
        price: number(node, "price"),
        currency: text(node, "currency"),
        hidden: node.attr_is("is_hidden", "true"),
        images: node
            .child("media")
            .map(|media| {
                media
                    .children_by("image")
                    .map(|image| ProductImage {
                        requested: text(image, "request_image_url"),
                        original: text(image, "original_image_url"),
                    })
                    .collect()
            })
            .unwrap_or_default(),
        review_status,
        can_appeal,
    }
}

pub(crate) fn parse_catalog(response: &Node) -> Catalog {
    let Some(catalog) = response.child("product_catalog") else { return Catalog::default() };
    Catalog {
        products: catalog.children_by("product").map(parse_product).collect(),
        next_cursor: catalog.child("paging").and_then(|paging| text(paging, "after")),
    }
}

pub(crate) fn parse_collections(response: &Node) -> Vec<ProductCollection> {
    let Some(list) = response.child("collections") else { return Vec::new() };
    list.children_by("collection")
        .map(|collection| {
            let (status, can_appeal) = review(collection);
            ProductCollection {
                id: text(collection, "id").unwrap_or_default(),
                name: text(collection, "name").unwrap_or_default(),
                products: collection.children_by("product").map(parse_product).collect(),
                status,
                can_appeal,
            }
        })
        .collect()
}

pub(crate) fn parse_order(response: &Node) -> Option<Order> {
    let order = response.child("order")?;
    let price = order.child("price");
    Some(Order {
        items: order
            .children_by("product")
            .map(|product| OrderItem {
                id: text(product, "id").unwrap_or_default(),
                name: text(product, "name").unwrap_or_default(),
                image_url: product.child("image").and_then(|image| text(image, "url")),
                price: number(product, "price"),
                currency: text(product, "currency"),
                quantity: number(product, "quantity").unwrap_or(1),
            })
            .collect(),
        total: price.and_then(|p| number(p, "total")),
        currency: price.and_then(|p| text(p, "currency")),
    })
}

pub(crate) fn product_node(id: Option<&str>, draft: &ProductDraft, image_urls: &[String], creating: bool) -> Node {
    let mut product = Node::new("product");
    let mut children = Vec::new();
    if let Some(id) = id {
        children.push(leaf("id", id));
    }
    let fields = [("name", &draft.name), ("description", &draft.description), ("retailer_id", &draft.retailer_id)];
    for (tag, value) in fields {
        if let Some(value) = value {
            children.push(leaf(tag, value.as_str()));
        }
    }
    if !image_urls.is_empty() {
        children.push(
            Node::new("media")
                .children(image_urls.iter().map(|url| Node::new("image").child_node(leaf("url", url.as_str())))),
        );
    }
    if let Some(price) = draft.price {
        children.push(leaf("price", price.to_string()));
    }
    if let Some(currency) = &draft.currency {
        children.push(leaf("currency", currency.as_str()));
    }
    if creating {
        match &draft.origin_country {
            Some(country) => {
                children.push(Node::new("compliance_info").child_node(leaf("country_code_origin", country.as_str())))
            }
            None => product.set_attr("compliance_category", "COUNTRY_ORIGIN_EXEMPT"),
        }
    }
    let hidden = if creating { Some(draft.hidden.unwrap_or(false)) } else { draft.hidden };
    if let Some(hidden) = hidden {
        product.set_attr("is_hidden", if hidden { "true" } else { "false" });
    }
    product.children(children)
}

fn business_profile_node(update: &BusinessProfileUpdate) -> Node {
    let mut children = Vec::new();
    let fields = [("address", &update.address), ("email", &update.email), ("description", &update.description)];
    for (tag, value) in fields {
        if let Some(value) = value {
            children.push(Node::new(tag).text(value.clone()));
        }
    }
    for site in update.websites.iter().flatten() {
        children.push(Node::new("website").text(site.clone()));
    }
    if let Some(hours) = &update.hours {
        let days = hours.days.iter().map(|(day, opening)| {
            let config = Node::new("business_hours_config").attr("day_of_week", day.as_str());
            match opening {
                OpeningHours::AllDay => config.attr("mode", "open_24h"),
                OpeningHours::ByAppointment => config.attr("mode", "appointment_only"),
                OpeningHours::Between { open_minute, close_minute } => config
                    .attr("mode", "specific_hours")
                    .attr("open_time", open_minute.to_string())
                    .attr("close_time", close_minute.to_string()),
            }
        });
        children.push(Node::new("business_hours").attr("timezone", hours.timezone.clone()).children(days));
    }
    Node::new("business_profile").attr("v", "3").attr("mutation_type", "delta").children(children)
}

impl Client {
    fn catalog_owner(&self, jid: Option<&Jid>) -> Result<Jid> {
        jid.cloned().or_else(|| self.me()).map(|j| j.to_non_device()).ok_or(Error::NotLoggedIn)
    }

    pub async fn catalog(&self, jid: Option<&Jid>, limit: u32, cursor: Option<&str>) -> Result<Catalog> {
        let owner = self.catalog_owner(jid)?;
        let mut params = vec![leaf("limit", limit.max(1).to_string())];
        params.extend(preview_size());
        if let Some(cursor) = cursor {
            params.push(leaf("after", cursor));
        }
        let request = self.server_iq("w:biz:catalog", "get").child_node(
            Node::new("product_catalog").attr("jid", owner).attr("allow_shop_source", "true").children(params),
        );
        Ok(parse_catalog(&self.query(request).await?))
    }

    pub async fn catalog_collections(&self, jid: Option<&Jid>, limit: u32) -> Result<Vec<ProductCollection>> {
        let owner = self.catalog_owner(jid)?;
        let limit = limit.max(1).to_string();
        let mut params = vec![leaf("collection_limit", limit.as_str()), leaf("item_limit", limit.as_str())];
        params.extend(preview_size());
        let request = self
            .server_iq("w:biz:catalog", "get")
            .attr("smax_id", "35")
            .child_node(Node::new("collections").attr("biz_jid", owner).children(params));
        Ok(parse_collections(&self.query(request).await?))
    }

    pub async fn order_details(&self, order_id: &str, token: &str) -> Result<Order> {
        let request = self.server_iq("fb:thrift_iq", "get").attr("smax_id", "5").child_node(
            Node::new("order")
                .attr("op", "get")
                .attr("id", order_id.to_owned())
                .children([Node::new("image_dimensions").children(preview_size()), leaf("token", token)]),
        );
        parse_order(&self.query(request).await?).ok_or_else(|| Error::other("order details missing"))
    }

    async fn catalog_image_url(&self, image: &CatalogImage) -> Result<String> {
        let data = match image {
            CatalogImage::Url(url) if url.contains(".whatsapp.net") => return Ok(url.clone()),
            CatalogImage::Url(url) => {
                let body = self.inner.http.get(url).send().await?.error_for_status()?.bytes().await?;
                if body.len() > MAX_REMOTE_IMAGE {
                    return Err(Error::Media(format!("product image at {url} is too large")));
                }
                body.to_vec()
            }
            CatalogImage::Data(data) => data.clone(),
        };
        let answer = self.upload_plain(data, PRODUCT_IMAGE_PATH).await?;
        let path = answer.direct_path.ok_or_else(|| Error::Media("product image upload without a path".into()))?;
        Ok(format!("{MEDIA_ORIGIN}{path}"))
    }

    async fn catalog_edit(&self, operation: &'static str, product: Node) -> Result<Product> {
        let mut children = vec![product];
        children.extend(preview_size());
        let request =
            self.server_iq("w:biz:catalog", "set").child_node(Node::new(operation).attr("v", "1").children(children));
        let response = self.query(request).await?;
        response
            .path(&[operation, "product"])
            .map(parse_product)
            .ok_or_else(|| Error::other(format!("{operation} answered without a product")))
    }

    pub async fn product_create(&self, draft: &ProductDraft) -> Result<Product> {
        let urls = try_join_all(draft.images.iter().map(|image| self.catalog_image_url(image))).await?;
        self.catalog_edit("product_catalog_add", product_node(None, draft, &urls, true)).await
    }

    pub async fn product_update(&self, product_id: &str, draft: &ProductDraft) -> Result<Product> {
        let urls = try_join_all(draft.images.iter().map(|image| self.catalog_image_url(image))).await?;
        self.catalog_edit("product_catalog_edit", product_node(Some(product_id), draft, &urls, false)).await
    }

    pub async fn product_delete(&self, product_ids: &[&str]) -> Result<u32> {
        let products = product_ids.iter().map(|id| Node::new("product").child_node(leaf("id", *id)));
        let request = self
            .server_iq("w:biz:catalog", "set")
            .child_node(Node::new("product_catalog_delete").attr("v", "1").children(products));
        let response = self.query(request).await?;
        Ok(response.child("product_catalog_delete").and_then(|n| n.attr_u64("deleted_count")).unwrap_or_default()
            as u32)
    }

    pub async fn update_business_profile(&self, update: &BusinessProfileUpdate) -> Result<()> {
        let request = self.server_iq("w:biz", "set").child_node(business_profile_node(update));
        self.query(request).await.map(drop)
    }

    pub async fn set_business_cover_photo(&self, image: Vec<u8>) -> Result<String> {
        let answer = self.upload_plain(image, COVER_PHOTO_PATH).await?;
        let id = answer.fbid().ok_or_else(|| Error::Media("cover photo upload without an id".into()))?;
        let cover = Node::new("cover_photo")
            .attr("id", id.clone())
            .attr("op", "update")
            .attr("token", answer.meta_hmac.clone().unwrap_or_default())
            .attr("ts", answer.ts().unwrap_or_default());
        let request = self
            .server_iq("w:biz", "set")
            .child_node(Node::new("business_profile").attr("v", "3").attr("mutation_type", "delta").child_node(cover));
        self.query(request).await?;
        Ok(id)
    }

    pub async fn remove_business_cover_photo(&self, id: &str) -> Result<()> {
        let cover = Node::new("cover_photo").attr("op", "delete").attr("id", id.to_owned());
        let request = self
            .server_iq("w:biz", "set")
            .child_node(Node::new("business_profile").attr("v", "3").attr("mutation_type", "delta").child_node(cover));
        self.query(request).await.map(drop)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn product_xml(id: &str, hidden: bool) -> Node {
        Node::new("product").attr("is_hidden", if hidden { "true" } else { "false" }).children([
            leaf("id", id),
            leaf("name", "Taza Soblend"),
            leaf("price", "149000"),
            leaf("currency", "MXN"),
            Node::new("media").child_node(Node::new("image").children([
                leaf("request_image_url", "https://mmg.whatsapp.net/a"),
                leaf("original_image_url", "https://mmg.whatsapp.net/b"),
            ])),
            Node::new("status_info").children([leaf("status", "APPROVED"), leaf("can_appeal", "false")]),
        ])
    }

    #[test]
    fn parses_catalog_pages() {
        let response = Node::new("iq").child_node(Node::new("product_catalog").children([
            product_xml("1", false),
            product_xml("2", true),
            Node::new("paging").child_node(leaf("after", "cursor-2")),
        ]));
        let catalog = parse_catalog(&response);
        assert_eq!(catalog.products.len(), 2);
        assert_eq!(catalog.next_cursor.as_deref(), Some("cursor-2"));
        let first = &catalog.products[0];
        assert_eq!(first.price, Some(149_000));
        assert_eq!(first.review_status.as_deref(), Some("APPROVED"));
        assert_eq!(first.images[0].original.as_deref(), Some("https://mmg.whatsapp.net/b"));
        assert!(catalog.products[1].hidden);
    }

    #[test]
    fn builds_product_nodes_like_the_official_client() {
        let draft = ProductDraft { name: Some("Taza".into()), price: Some(99_000), ..Default::default() };
        let created = product_node(None, &draft, &["https://mmg.whatsapp.net/x".into()], true);
        assert!(created.attr_is("compliance_category", "COUNTRY_ORIGIN_EXEMPT"));
        assert!(created.attr_is("is_hidden", "false"));
        assert_eq!(
            created.path(&["media", "image", "url"]).and_then(Node::content_string).as_deref(),
            Some("https://mmg.whatsapp.net/x")
        );
        let edited = product_node(Some("42"), &draft, &[], false);
        assert!(!edited.has("compliance_category") && !edited.has("is_hidden"));
        assert_eq!(text(&edited, "id").as_deref(), Some("42"));
    }

    #[test]
    fn parses_orders() {
        let response = Node::new("iq").child_node(Node::new("order").children([
            Node::new("product").children([
                leaf("id", "7"),
                leaf("name", "Playera"),
                leaf("price", "250000"),
                leaf("currency", "MXN"),
                leaf("quantity", "3"),
                Node::new("image").child_node(leaf("url", "https://mmg.whatsapp.net/p")),
            ]),
            Node::new("price").children([leaf("total", "750000"), leaf("currency", "MXN")]),
        ]));
        let order = parse_order(&response).unwrap();
        assert_eq!(order.items[0].quantity, 3);
        assert_eq!(order.total, Some(750_000));
        assert_eq!(order.items[0].image_url.as_deref(), Some("https://mmg.whatsapp.net/p"));
    }
}
