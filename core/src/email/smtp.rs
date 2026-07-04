use lettre::message::{
    header::{ContentDisposition, ContentType},
    MultiPart, SinglePart,
};
use lettre::{
    transport::smtp::authentication::Credentials, AsyncSmtpTransport, AsyncTransport,
    Message as LettreMessage, Tokio1Executor,
};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum SmtpError {
    #[error("smtp error: {0}")]
    Transport(String),
    #[error("message build error: {0}")]
    Build(String),
}

#[derive(Debug, Clone)]
pub struct SmtpConfig {
    pub server: String,
    pub port: u16,
    pub username: String,
    pub password: String,
}

pub struct SmtpSender {
    config: SmtpConfig,
}

impl SmtpSender {
    pub fn new(config: SmtpConfig) -> Self {
        Self { config }
    }

    pub async fn send(
        &self,
        from: (&str, &str),
        to: &str,
        body: Vec<u8>,
        attachments: Vec<(&str, Vec<u8>)>,
    ) -> Result<(), SmtpError> {
        let mut multipart = MultiPart::mixed().singlepart(
            SinglePart::builder()
                .header(ContentType::parse("text/plain").unwrap())
                .body(body),
        );

        for (fname, data) in attachments {
            let part = SinglePart::builder()
                .header(ContentType::parse("application/octet-stream").unwrap())
                .header(ContentDisposition::attachment(fname))
                .body(data);
            multipart = multipart.singlepart(part);
        }

        let email = LettreMessage::builder()
            .from(
                format!("{} <{}>", from.1, from.0)
                    .parse::<lettre::message::Mailbox>()
                    .map_err(|e| SmtpError::Build(e.to_string()))?,
            )
            .to(to
                .parse::<lettre::message::Mailbox>()
                .map_err(|e| SmtpError::Build(e.to_string()))?)
            .subject("")
            .multipart(multipart)
            .map_err(|e| SmtpError::Build(e.to_string()))?;

        let creds = Credentials::new(self.config.username.clone(), self.config.password.clone());

        let mailer: AsyncSmtpTransport<Tokio1Executor> =
            AsyncSmtpTransport::<Tokio1Executor>::relay(&self.config.server)
                .map_err(|e| SmtpError::Transport(e.to_string()))?
                .port(self.config.port)
                .credentials(creds)
                .build();

        mailer
            .send(email)
            .await
            .map_err(|e| SmtpError::Transport(e.to_string()))?;

        Ok(())
    }
}
