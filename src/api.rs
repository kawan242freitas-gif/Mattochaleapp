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

pub fn fetch_business_data(token: &str) -> Result<serde_json::Value> {
    Client::new()
        .get(format!("{API_BASE}/business-data"))
        .bearer_auth(token)
        .send()
        .context("Não foi possível conectar aos dados da empresa")?
        .error_for_status()
        .context("O servidor não conseguiu carregar reservas e financeiro")?
        .json()
        .context("O servidor retornou dados empresariais inválidos")
}

pub fn sync_business_records(token: &str, collection: &str, records: &serde_json::Value) -> Result<()> {
    Client::new()
        .post(format!("{API_BASE}/business-data"))
        .bearer_auth(token)
        .json(&serde_json::json!({ "action": "sync", "collection": collection, "records": records }))
        .send()
        .context("Não foi possível enviar os dados ao servidor")?
        .error_for_status()
        .context("O servidor recusou a sincronização dos dados")?;
    Ok(())
}

pub fn delete_business_record(token: &str, collection: &str, id: &str) -> Result<()> {
    Client::new()
        .delete(format!("{API_BASE}/business-data"))
        .query(&[("collection", collection), ("id", id)])
        .bearer_auth(token)
        .send()
        .context("Não foi possível excluir o registro no servidor")?
        .error_for_status()
        .context("O servidor recusou a exclusão do registro")?;
    Ok(())
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
