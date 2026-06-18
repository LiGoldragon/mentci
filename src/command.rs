use std::path::PathBuf;

use crate::configuration::{ConfigurationFile, DaemonConfiguration};
use crate::daemon::Daemon;
use crate::{Error, Result};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DaemonCommand {
    arguments: Vec<String>,
}

impl DaemonCommand {
    pub fn from_environment() -> Self {
        Self {
            arguments: std::env::args().skip(1).collect(),
        }
    }

    pub fn from_arguments<Arguments, Argument>(arguments: Arguments) -> Self
    where
        Arguments: IntoIterator<Item = Argument>,
        Argument: Into<String>,
    {
        Self {
            arguments: arguments.into_iter().map(Into::into).collect(),
        }
    }

    pub fn configuration(&self) -> Result<DaemonConfiguration> {
        let path = self.startup_path()?;
        if path
            .extension()
            .is_some_and(|extension| extension == "nota")
        {
            return Err(Error::StartupNotaRejected(path));
        }
        ConfigurationFile::new(path)
            .configuration()
            .map(DaemonConfiguration::new)
    }

    pub fn run(&self) -> Result<()> {
        Daemon::from_configuration(self.configuration()?)?.run()
    }

    fn startup_path(&self) -> Result<PathBuf> {
        match self.arguments.as_slice() {
            [path] => Ok(PathBuf::from(path)),
            _ => Err(Error::StartupArgumentCount),
        }
    }
}
