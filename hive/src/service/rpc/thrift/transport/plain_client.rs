const SEPARATOR: u8 = 0;

#[derive(Debug)]
pub struct PlainClient {
    authzid: Option<String>,
    authid: String,
    password: Vec<u8>,
    completed: bool,
}

impl Drop for PlainClient {
    fn drop(&mut self) {
        self.clear_password();
    }
}

impl PlainClient {
    pub fn new(authzid: Option<String>, authid: String, password: Vec<u8>) -> Self {
        Self {
            authzid,
            authid,
            password,
            completed: false,
        }
    }

    pub fn mechanism_name(&self) -> &str {
        "PLAIN"
    }

    pub fn has_initial_response(&self) -> bool {
        true
    }

    pub fn step(&mut self, _input: &[u8]) -> anyhow::Result<Vec<u8>> {
        if self.completed {
            Err(anyhow::anyhow!("PLAIN authentication already completed"))?
        }
        self.completed = true;

        let mut output = vec![];
        if let Some(authzid) = &self.authzid {
            output.append(&mut authzid.as_bytes().to_owned());
        }
        output.push(SEPARATOR);
        output.append(&mut self.authid.as_bytes().to_owned());
        output.push(SEPARATOR);
        output.append(&mut self.password);

        self.clear_password();
        Ok(output)
    }

    pub fn is_complete(&self) -> bool {
        self.completed
    }

    fn clear_password(&mut self) {
        for i in 0..self.password.len() {
            self.password[i] = 0;
        }
    }
}
