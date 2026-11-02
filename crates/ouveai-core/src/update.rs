//! Verificação de atualização (D5): uma consulta ao GitHub Releases quando a pessoa clica ou,
//! se ela ligar "Avisar quando sair versão nova" (0.9.3), ao abrir o app, no máximo uma vez
//! a cada 24 h. Nada é baixado nem instalado aqui; o app só abre a página da versão.

use crate::net::{NetConfig, NetError, agent};

/// Repositório público onde saem os instaladores.
pub const REPO: &str = "freire-andre/OuveAI";

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Release {
    /// Sem o "v" da tag (ex.: "0.9.1").
    pub version: String,
    /// Página da versão no GitHub (notas e instalador).
    pub url: String,
}

/// Última versão publicada, ou `None` se ainda não há nenhuma.
pub fn latest(cfg: &NetConfig, current: &str) -> Result<Option<Release>, NetError> {
    let url = format!("https://api.github.com/repos/{REPO}/releases/latest");
    let mut resp = agent(cfg)
        .get(&url)
        .header("User-Agent", format!("OuveAI/{current}"))
        .header("Accept", "application/vnd.github+json")
        .call()
        .map_err(|e| NetError::Net(e.to_string()))?;
    let status = resp.status().as_u16();
    log::info!("verificação de atualização: {status}");
    match status {
        200 => {}
        404 => return Ok(None),
        s => return Err(NetError::Status(s)),
    }
    let body = resp.body_mut().read_to_string().map_err(|e| NetError::Net(e.to_string()))?;
    parse(&body).map(Some).ok_or_else(|| NetError::Net("resposta inesperada do GitHub".into()))
}

fn parse(body: &str) -> Option<Release> {
    let v: serde_json::Value = serde_json::from_str(body).ok()?;
    let tag = v.get("tag_name")?.as_str()?;
    let url = v.get("html_url")?.as_str()?;
    // Só abre página do próprio repositório.
    if !url.starts_with(&format!("https://github.com/{REPO}/")) {
        return None;
    }
    Some(Release { version: tag.trim_start_matches(['v', 'V']).to_string(), url: url.to_string() })
}

/// Intervalo mínimo entre verificações automáticas.
pub const AUTO_INTERVAL_SECS: u64 = 24 * 3600;

/// Já passou o intervalo desde a última verificação automática? Relógio que voltou no
/// tempo (`now < last`) conta como vencido, para não ficar dias sem verificar.
pub fn due(last_check: Option<u64>, now: u64) -> bool {
    match last_check {
        None => true,
        Some(t) => now < t || now - t >= AUTO_INTERVAL_SECS,
    }
}

/// O aviso a mostrar: a última versão, se for mais nova que a instalada e não for a que a
/// pessoa mandou ignorar.
pub fn notice(latest: Option<&Release>, current: &str, skipped: Option<&str>) -> Option<Release> {
    latest.filter(|l| is_newer(&l.version, current) && skipped != Some(l.version.as_str())).cloned()
}

/// `candidate` é mais nova que `current`? Compara os números (1.10 > 1.9); um sufixo
/// ("-beta") vale menos que a versão sem sufixo.
pub fn is_newer(candidate: &str, current: &str) -> bool {
    fn key(v: &str) -> (Vec<u64>, bool) {
        let (nums, pre) = match v.split_once('-') {
            Some((n, _)) => (n, true),
            None => (v, false),
        };
        (nums.split('.').map(|p| p.parse().unwrap_or(0)).collect(), !pre)
    }
    let (mut a, ra) = key(candidate);
    let (mut b, rb) = key(current);
    let n = a.len().max(b.len());
    a.resize(n, 0);
    b.resize(n, 0);
    (a, ra) > (b, rb)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rel(v: &str) -> Release {
        Release { version: v.into(), url: format!("https://github.com/{REPO}/releases/tag/v{v}") }
    }

    #[test]
    fn verificacao_automatica_uma_vez_por_dia() {
        assert!(due(None, 1_000));
        assert!(!due(Some(1_000), 1_000 + AUTO_INTERVAL_SECS - 1));
        assert!(due(Some(1_000), 1_000 + AUTO_INTERVAL_SECS));
        // Relógio voltou no tempo: verifica de novo.
        assert!(due(Some(1_000), 999));
    }

    #[test]
    fn aviso_so_para_versao_nova_e_nao_ignorada() {
        assert_eq!(notice(Some(&rel("0.9.4")), "0.9.3", None), Some(rel("0.9.4")));
        assert_eq!(notice(Some(&rel("0.9.3")), "0.9.3", None), None);
        assert_eq!(notice(Some(&rel("0.9.2")), "0.9.3", None), None);
        assert_eq!(notice(Some(&rel("0.9.4")), "0.9.3", Some("0.9.4")), None);
        // Ignorar a 0.9.4 não esconde a 0.9.5.
        assert_eq!(notice(Some(&rel("0.9.5")), "0.9.3", Some("0.9.4")), Some(rel("0.9.5")));
        assert_eq!(notice(None, "0.9.3", None), None);
    }

    #[test]
    fn compara_versoes() {
        assert!(is_newer("0.9.1", "0.9.0"));
        assert!(is_newer("1.10.0", "1.9.3"));
        assert!(is_newer("1.0", "0.9.9"));
        assert!(is_newer("1.0.0", "1.0.0-beta"));
        assert!(!is_newer("0.9.0", "0.9.0"));
        assert!(!is_newer("0.9", "0.9.0"));
        assert!(!is_newer("0.8.9", "0.9.0"));
        assert!(!is_newer("1.0.0-beta", "1.0.0"));
    }

    #[test]
    fn le_a_resposta_do_github_e_recusa_link_de_fora() {
        let ok = format!(r#"{{"tag_name":"v0.9.1","html_url":"https://github.com/{REPO}/releases/tag/v0.9.1","assets":[]}}"#);
        assert_eq!(
            parse(&ok),
            Some(Release { version: "0.9.1".into(), url: format!("https://github.com/{REPO}/releases/tag/v0.9.1") })
        );
        assert_eq!(parse(r#"{"tag_name":"v9","html_url":"https://exemplo.com/x"}"#), None);
        assert_eq!(parse("não é json"), None);
    }
}
