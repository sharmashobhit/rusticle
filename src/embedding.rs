use fastembed::{InitOptions, TextEmbedding};

// #[cfg(not(test))]
pub(crate) async fn embed(
    config: &crate::config::Config,
    documents: Vec<&str>,
) -> Result<Vec<Vec<f32>>, fastembed::Error> {
    let model = TextEmbedding::try_new(
        InitOptions::new(config.embedding.model.clone()).with_show_download_progress(true),
    )?;

    let embeddings = model.embed(documents, None)?;

    Ok(embeddings)
}

// #[cfg(test)]
// pub(crate) async fn embed(
//     _config: &crate::config::Config,
// ) -> Result<Vec<Vec<f32>>, fastembed::Error> {
//     Ok(vec![vec![0.0, 0.1, 0.2, 0.3]])
// }

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_embed() {
        let config = crate::config::Config::default();
        embed(&config, vec!["hello", "world"]);
    }
}
