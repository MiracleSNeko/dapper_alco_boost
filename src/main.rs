#[cfg(feature = "meta_collect")]
mod commands_impl {
    use anyhow::{anyhow, Result};
    use meta_collect::wgse_command;

    #[wgse_command(0x01, "Nope")]
    fn execute_nope() -> Result<()> {
        println!("Nope!");
        Ok(())
    }

    #[wgse_command(0xFF, "Panic")]
    /// doc here
    pub fn panic() -> Result<()> {
        println!("Panic!");
        unreachable!()
    }
}

#[cfg(feature = "meta_collect")]
pub use commands_impl::*;

#[cfg(not(feature = "meta_collect"))]
mod commands {
    use anyhow::{anyhow, Result};
    use enum_dispatch::enum_dispatch;
    #[cfg(not(feature = "meta_init"))]
    use meta_gen::generate_wgse_commands;
    #[cfg(feature = "meta_init")]
    use meta_gen::wgse_command_interface;

    // definition of instruction related functions' signature
    #[cfg(feature = "meta_init")]
    pub trait WgseCommandInterface {
        #[wgse_command_interface]
        fn execute(&self) -> Result<()>;
    }

    #[cfg(not(feature = "meta_init"))]
    #[enum_dispatch]
    pub trait WgseCommandInterface {
        #[allow(unused_variables)]
        // summary doc here
        fn execute(&self) -> Result<()>;
    }

    /// Auto-completed by proc-macro
    #[cfg(not(feature = "meta_init"))]
    #[generate_wgse_commands(WgseCommandInterface)]
    pub enum WgseCommands {}
}

#[cfg(not(feature = "meta_collect"))]
pub use commands::*;

use anyhow::Result;

fn main() -> Result<()> {
    #[cfg(not(any(feature = "meta_init", feature = "meta_collect")))]
    {
        let nope = WgseCommands::Nope(Nope);
        nope.execute()?;
    }
    println!("hello, world!");
    Ok(())
}
