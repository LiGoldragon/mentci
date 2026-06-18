use std::path::{Path, PathBuf};

use meta_signal_mentci::{Input as MetaInput, MentciDaemonConfiguration};

use crate::{Error, Result};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConfigurationFile {
    path: PathBuf,
}

impl ConfigurationFile {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    pub fn as_path(&self) -> &Path {
        &self.path
    }

    pub fn configuration(&self) -> Result<MentciDaemonConfiguration> {
        let bytes = std::fs::read(&self.path).map_err(|source| Error::ConfigurationRead {
            path: self.path.clone(),
            source,
        })?;
        let (_route, input) = MetaInput::decode_signal_frame(&bytes)?;
        match input {
            MetaInput::Configure(configuration) => Ok(configuration),
        }
    }

    pub fn write_configuration(&self, configuration: &MentciDaemonConfiguration) -> Result<()> {
        let input = MetaInput::Configure(configuration.clone());
        let bytes = input.encode_signal_frame()?;
        std::fs::write(&self.path, bytes).map_err(|source| Error::ConfigurationWrite {
            path: self.path.clone(),
            source,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DaemonConfiguration {
    inner: MentciDaemonConfiguration,
}

impl DaemonConfiguration {
    pub fn new(inner: MentciDaemonConfiguration) -> Self {
        Self { inner }
    }

    pub fn socket_path(&self) -> Result<&Path> {
        Ok(Path::new(self.inner.socket_path.payload().as_ref()))
    }

    pub fn home_criome_socket_path(&self) -> Result<&Path> {
        Ok(Path::new(self.inner.home_criome_socket.payload().as_ref()))
    }

    pub fn into_inner(self) -> MentciDaemonConfiguration {
        self.inner
    }
}
