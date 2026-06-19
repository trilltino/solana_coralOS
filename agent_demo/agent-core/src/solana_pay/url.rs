use serde::{Deserialize, Serialize};

/// Fields of a Solana Pay transfer request URL.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TransferUrlFields {
    pub recipient: String,
    pub amount: Option<f64>,
    pub spl_token: Option<String>,
    pub reference: Option<Vec<String>>,
    pub label: Option<String>,
    pub message: Option<String>,
    pub memo: Option<String>,
}

/// Fields of a Solana Pay transaction request URL.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TransactionRequestUrlFields {
    pub link: String,
    pub label: Option<String>,
    pub message: Option<String>,
}

/// Parsed Solana Pay URL — either a transfer or a transaction request.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ParsedUrl {
    #[serde(rename = "transfer")]
    Transfer(TransferUrlFields),
    #[serde(rename = "transaction")]
    Transaction(TransactionRequestUrlFields),
}

/// Encode a Solana Pay transfer URL following the spec.
pub fn encode_transfer_url(fields: &TransferUrlFields) -> String {
    let mut url = format!("solana:{}", fields.recipient);
    let mut params = Vec::new();

    if let Some(amount) = fields.amount {
        let s = format!("{:.10}", amount)
            .trim_end_matches('0')
            .trim_end_matches('.')
            .to_string();
        params.push(format!("amount={}", s));
    }

    if let Some(ref token) = fields.spl_token {
        params.push(format!("spl-token={}", token));
    }

    if let Some(ref refs) = fields.reference {
        for r in refs {
            params.push(format!("reference={}", r));
        }
    }

    if let Some(ref label) = fields.label {
        params.push(format!("label={}", urlencoding::encode(label)));
    }

    if let Some(ref message) = fields.message {
        params.push(format!("message={}", urlencoding::encode(message)));
    }

    if let Some(ref memo) = fields.memo {
        params.push(format!("memo={}", urlencoding::encode(memo)));
    }

    if !params.is_empty() {
        url.push('?');
        url.push_str(&params.join("&"));
    }

    url
}

/// Encode a Solana Pay transaction request URL.
pub fn encode_transaction_request_url(fields: &TransactionRequestUrlFields) -> String {
    let mut url = format!("solana:{}", urlencoding::encode(&fields.link));
    let mut params = Vec::new();

    if let Some(ref label) = fields.label {
        params.push(format!("label={}", urlencoding::encode(label)));
    }

    if let Some(ref message) = fields.message {
        params.push(format!("message={}", urlencoding::encode(message)));
    }

    if !params.is_empty() {
        url.push('?');
        url.push_str(&params.join("&"));
    }

    url
}

/// Parse a Solana Pay URL (transfer or transaction request).
pub fn parse_url(url_str: &str) -> anyhow::Result<ParsedUrl> {
    let url = url::Url::parse(url_str)?;

    if url.scheme() != "solana" {
        anyhow::bail!("Invalid protocol: expected 'solana:'");
    }

    let pathname = url.path().to_string();

    // Transaction request URLs contain ":" or "%" (URL-encoded https://)
    if pathname.contains(':') || pathname.contains('%') {
        let link = urlencoding::decode(&pathname)?;
        let label = url.query_pairs().find(|(k, _)| k == "label").map(|(_, v)| v.to_string());
        let message = url.query_pairs().find(|(k, _)| k == "message").map(|(_, v)| v.to_string());

        return Ok(ParsedUrl::Transaction(TransactionRequestUrlFields {
            link: link.to_string(),
            label,
            message,
        }));
    }

    // Otherwise it's a transfer request
    let recipient = pathname;
    let mut amount: Option<f64> = None;
    let mut spl_token: Option<String> = None;
    let mut references: Vec<String> = Vec::new();
    let mut label: Option<String> = None;
    let mut message: Option<String> = None;
    let mut memo: Option<String> = None;

    for (key, value) in url.query_pairs() {
        match key.as_ref() {
            "amount" => {
                amount = Some(value.parse()?);
            }
            "spl-token" => {
                spl_token = Some(value.to_string());
            }
            "reference" => {
                references.push(value.to_string());
            }
            "label" => {
                label = Some(value.to_string());
            }
            "message" => {
                message = Some(value.to_string());
            }
            "memo" => {
                memo = Some(value.to_string());
            }
            _ => {}
        }
    }

    Ok(ParsedUrl::Transfer(TransferUrlFields {
        recipient,
        amount,
        spl_token,
        reference: if references.is_empty() { None } else { Some(references) },
        label,
        message,
        memo,
    }))
}
