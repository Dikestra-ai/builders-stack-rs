use anyhow::{anyhow, Result};
use serde::Serialize;

#[derive(Debug, Clone)]
pub enum EmailTemplate {
    Welcome,
    VerifyEmail,
    DripDay3,
}

#[derive(Debug, Clone)]
pub struct EmailArgs {
    pub to: String,
    pub template: EmailTemplate,
    pub name: String,
    /// Extra data: verify_url, email_verification_url, etc.
    pub extra: std::collections::HashMap<String, String>,
}

pub async fn send_email(args: EmailArgs) -> Result<()> {
    let api_key = match std::env::var("RESEND_API_KEY").ok() {
        Some(k) if !k.is_empty() => k,
        _ => {
            tracing::warn!("RESEND_API_KEY not set — email send skipped");
            return Ok(());
        }
    };

    let from = std::env::var("RESEND_FROM")
        .unwrap_or_else(|_| "noreply@stack.example.com".to_string());

    let (subject, html) = render_template(&args.template, &args.name, &args.extra)?;

    #[derive(Serialize)]
    struct ResendPayload {
        from: String,
        to: Vec<String>,
        subject: String,
        html: String,
    }

    let payload = ResendPayload {
        from,
        to: vec![args.to],
        subject,
        html,
    };

    let resp = reqwest::Client::new()
        .post("https://api.resend.com/emails")
        .bearer_auth(api_key)
        .json(&payload)
        .send()
        .await?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(anyhow!("Resend API error {}: {}", status, body));
    }

    Ok(())
}

fn render_template(
    template: &EmailTemplate,
    name: &str,
    extra: &std::collections::HashMap<String, String>,
) -> Result<(String, String)> {
    match template {
        EmailTemplate::Welcome => {
            let url = extra.get("emailVerificationUrl").map(|s| s.as_str()).unwrap_or("#");
            Ok((
                format!("Welcome to Builder's Stack, {}!", name),
                format!(
                    "<h1>Welcome, {}!</h1><p>Verify your email: <a href='{}'>{}</a></p>",
                    name, url, url
                ),
            ))
        }
        EmailTemplate::VerifyEmail => {
            let url = extra.get("verifyUrl").map(|s| s.as_str()).unwrap_or("#");
            Ok((
                "Verify your email".to_string(),
                format!(
                    "<h1>Hi {}!</h1><p>Please verify your email: <a href='{}'>{}</a></p>",
                    name, url, url
                ),
            ))
        }
        EmailTemplate::DripDay3 => {
            let email = extra.get("recipientEmail").map(|s| s.as_str()).unwrap_or("");
            Ok((
                "How's it going?".to_string(),
                format!(
                    "<h1>Hi {}!</h1><p>Hope you're enjoying Builder's Stack. Any questions? Reply to this email ({})</p>",
                    name, email
                ),
            ))
        }
    }
}
