use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Product {
    pub id: String,
    pub name: String,
    pub label: String,
    pub price: String,
    #[serde(rename = "promotionalPrice")]
    pub promotional_price: Option<String>,
    #[serde(rename = "stockQuantity")]
    pub stock_quantity: Option<u32>,
    pub description: String,
    pub tone: String,
    pub category: String,
    #[serde(rename = "imageUrl")]
    pub image_url: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct CartEntry {
    pub product_id: String,
    pub quantity: u32,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct PersistedState {
    pub cart: Vec<CartEntry>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Page {
    #[default]
    Home,
    Buscar,
    Carrinho,
    Configuracoes,
    Detalhe,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CategoryFilter {
    #[default]
    Todos,
    Adega,
    Frigobar,
}
