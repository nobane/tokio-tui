// tokio-tui/src/widgets/input/command_set.rs
use anyhow::Result;
use clap::Parser;
use std::{collections::HashMap, future::Future, pin::Pin, sync::Arc};

// Type-erased command interface
pub trait ErasedCommand: Send + Sync {
    fn execute(
        &self,
        args: Vec<String>,
    ) -> Pin<Box<dyn Future<Output = Result<Option<String>>> + Send + Sync + '_>>;
    fn name(&self) -> &str;
    fn help_msg(&self) -> &str;
}

pub trait InputCommand<C: Clone + Send + Sync + 'static>: Send + Sync {
    fn execute(
        &self,
        context: CommandContext<Vec<String>, C>,
    ) -> Pin<Box<dyn Future<Output = Result<Option<String>>> + Send + Sync + '_>>;
    fn name(&self) -> &str;
    fn help_msg(&self) -> &str;
}

// Command data for the type-erased command set
struct CommandData {
    full_help: String,
    command_map: HashMap<String, Arc<dyn ErasedCommand>>,
    help_map: HashMap<String, String>,
}

// Type-erased command set
#[derive(Clone)]
pub struct CommandSet {
    data: Arc<CommandData>,
}

impl CommandSet {
    pub fn full_help(&self) -> &String {
        &self.data.full_help
    }

    pub async fn parse_line(&self, line: impl AsRef<str>) -> Option<String> {
        let line = line.as_ref().trim();
        let args: Vec<String> = line.split_whitespace().map(String::from).collect();
        if args.is_empty() {
            return None;
        }

        let command_name = &args[0];
        if command_name == "help" {
            let help_text = if args.len() == 1 {
                self.data.full_help.clone()
            } else {
                match self.data.help_map.get(&args[1]) {
                    Some(help_msg) => help_msg.clone(),
                    None => format!("No such command '{}'", args[1]),
                }
            };

            Some(help_text)
        } else {
            match self.data.command_map.get(command_name) {
                Some(command) => {
                    let command = command.clone();
                    match command.execute(args).await {
                        Ok(output) => output,
                        Err(e) => Some(format!("Error executing command: {e}")),
                    }
                }
                None => Some(format!("invalid command '{command_name}' (try `help`)")),
            }
        }
    }
}

// Generic command set builder
pub struct CommandSetBuilder<State: Clone + Send + Sync + 'static> {
    commands: Vec<Arc<dyn InputCommand<State>>>,
}

impl<State: Clone + Send + Sync + 'static> CommandSetBuilder<State> {
    pub fn new() -> Self {
        Self {
            commands: Vec::new(),
        }
    }

    pub fn add_command<C: InputCommand<State> + 'static>(mut self, command: C) -> Self {
        self.commands.push(Arc::new(command));
        self
    }

    pub fn add_simple<F, Fut>(self, name: &str, help_msg: &str, executor: F) -> Self
    where
        F: Fn(CommandContext<Vec<String>, State>) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<Option<String>>> + Send + Sync + 'static,
    {
        self.add_command(SimpleCommand::new(name, help_msg, executor))
    }

    pub fn add_clap<T, F, Fut>(self, name: &str, executor: F) -> Self
    where
        T: Parser + Send + Sync + 'static,
        F: Fn(CommandContext<T, State>) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<Option<String>>> + Send + Sync + 'static,
    {
        self.add_command(ClapCommand::new(name, executor))
    }

    pub fn build(self, state: State) -> CommandSet {
        let mut command_map = HashMap::new();
        let mut help_map = HashMap::new();

        for command in self.commands {
            let name = command.name().to_string();
            help_map.insert(name.clone(), command.help_msg().to_string());

            // Create type-erased wrapper for this command
            let erased_command = ErasedCommandWrapper {
                inner: command,
                state: state.clone(),
            };

            command_map.insert(name, Arc::new(erased_command) as Arc<dyn ErasedCommand>);
        }

        let mut commands: Vec<&str> = help_map.keys().map(AsRef::as_ref).collect();
        commands.sort();

        let indent = "    ";
        let commands = commands.join(&format!("\n{indent}"));

        let help_header = "Available commands:";
        let help_footer = "Type `help <COMMAND>` for more information on a specific command";

        let full_help = format!("\n{help_header}\n\n{indent}{commands}\n\n{help_footer}\n");

        let data = Arc::new(CommandData {
            full_help,
            command_map,
            help_map,
        });

        CommandSet { data }
    }
}

impl<State: Clone + Send + Sync + 'static> Default for CommandSetBuilder<State> {
    fn default() -> Self {
        Self::new()
    }
}

// Wrapper that erases the state type from a command
struct ErasedCommandWrapper<S: Clone + Send + Sync + 'static> {
    inner: Arc<dyn InputCommand<S>>,
    state: S,
}

impl<S: Clone + Send + Sync + 'static> ErasedCommand for ErasedCommandWrapper<S> {
    fn execute(
        &self,
        args: Vec<String>,
    ) -> Pin<Box<dyn Future<Output = Result<Option<String>>> + Send + Sync + '_>> {
        let context = CommandContext {
            args,
            state: self.state.clone(),
        };
        self.inner.execute(context)
    }

    fn name(&self) -> &str {
        self.inner.name()
    }

    fn help_msg(&self) -> &str {
        self.inner.help_msg()
    }
}

pub struct CommandContext<Args, State> {
    pub args: Args,
    pub state: State,
}

pub type SimpleContext<State> = CommandContext<Vec<String>, State>;

type CommandFut = Pin<Box<dyn Future<Output = Result<Option<String>>> + Send + Sync>>;

pub struct SimpleCommand<C: Clone + Send + Sync + 'static> {
    name: String,
    help_msg: String,
    executor: Arc<dyn Fn(SimpleContext<C>) -> CommandFut + Send + Sync>,
}

impl<C: Clone + Send + Sync + 'static> SimpleCommand<C> {
    pub fn new<CommandFn, CommandFuture>(name: &str, help_msg: &str, executor: CommandFn) -> Self
    where
        CommandFn: Fn(CommandContext<Vec<String>, C>) -> CommandFuture + Send + Sync + 'static,
        CommandFuture: Future<Output = Result<Option<String>>> + Send + Sync + 'static,
    {
        Self {
            name: name.to_string(),
            help_msg: help_msg.to_string(),
            executor: Arc::new(move |context| Box::pin(executor(context))),
        }
    }
}

impl<C: Clone + Send + Sync + 'static> InputCommand<C> for SimpleCommand<C> {
    fn execute(
        &self,
        context: CommandContext<Vec<String>, C>,
    ) -> Pin<Box<dyn Future<Output = Result<Option<String>>> + Send + Sync>> {
        let CommandContext { args, state } = context;
        let args = args[1..].to_vec();
        (self.executor)(CommandContext { args, state })
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn help_msg(&self) -> &str {
        &self.help_msg
    }
}

pub struct ClapCommand<T: Parser + Send + Sync + 'static, C: Clone + Send + Sync + 'static> {
    name: String,
    help_msg: String,
    executor: Arc<dyn Fn(CommandContext<T, C>) -> CommandFut + Send + Sync>,
}

impl<ClapParser: Parser + Send + Sync + 'static, C: Clone + Send + Sync + 'static>
    ClapCommand<ClapParser, C>
{
    pub fn new<CommandFn, CommandFuture>(name: &str, executor: CommandFn) -> Self
    where
        CommandFn: Fn(CommandContext<ClapParser, C>) -> CommandFuture,
        CommandFn: Send + Sync + 'static,
        CommandFuture: Future<Output = Result<Option<String>>> + Send + Sync + 'static,
    {
        Self {
            help_msg: ClapParser::command()
                .name(clap::builder::Str::from(name.to_string()))
                .render_help()
                .to_string(),
            name: name.to_string(),
            executor: Arc::new(move |context| Box::pin(executor(context))),
        }
    }
}

impl<P: Parser + Send + Sync + 'static, C: Clone + Send + Sync + 'static> InputCommand<C>
    for ClapCommand<P, C>
{
    fn execute(
        &self,
        context: CommandContext<Vec<String>, C>,
    ) -> Pin<Box<dyn Future<Output = Result<Option<String>>> + Send + Sync + '_>> {
        Box::pin(async move {
            match P::try_parse_from(&context.args) {
                Ok(clap_args) => {
                    (self.executor)(CommandContext {
                        args: clap_args,
                        state: context.state,
                    })
                    .await
                }
                Err(e) => match e.kind() {
                    clap::error::ErrorKind::DisplayHelp
                    | clap::error::ErrorKind::DisplayVersion
                    | clap::error::ErrorKind::DisplayHelpOnMissingArgumentOrSubcommand => Ok(None),
                    _ => Err(e.into()),
                },
            }
        })
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn help_msg(&self) -> &str {
        &self.help_msg
    }
}
