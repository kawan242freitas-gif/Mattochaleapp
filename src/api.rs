use anyhow::{Context, Result};
use reqwest::blocking::Client;
use serde::Deserialize;

use crate::model::Product;

pub const API_BASE: &str = "https://matto-chales.vercel.app/api";
const SITE_BASE: &str = "https://matto-chales.vercel.app";
pub const HERO_IMAGE_URL: &str =
    "https://r2.fivemanage.com/vLUsF9vzqBOo7DSFHERFX/CestadeVinhosMattoChals.png";

pub fn fetch_products() -> Result<Vec<Product>> {
    Client::new()
        .get(format!("{API_BASE}/products"))
        .send()
        .context("Não foi possível conectar ao catálogo")?
        .error_for_status()
        .context("O servidor não conseguiu carregar os produtos")?
        .json::<Vec<Product>>()
        .context("O catálogo retornou dados inválidos")
}

pub fn fetch_image(url: &str) -> Result<image::RgbaImage> {
    let bytes = Client::new()
        .get(url)
        .send()
        .context("Não foi possível baixar a imagem")?
        .error_for_status()?
        .bytes()?;
    Ok(image::load_from_memory(&bytes)?.to_rgba8())
}

#[derive(Debug, Deserialize)]
pub struct CreatedOrderResponse {
    pub token: String,
    pub order: Option<serde_json::Value>,
}

pub fn order_detail_url(token: &str) -> String {
    format!("{SITE_BASE}/?pedido={token}")
}
