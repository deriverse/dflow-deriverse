use bytemuck::{Zeroable, bytes_of};
use drv_models::{
    constants::seeds::DRVS_SEED,
    instruction_constants::{
        DepositInstruction, DrvInstruction, MigrateInstrInstruction, MigrateTokenInstruction,
        MoveFundsInstruction,
    },
    instruction_data::{DepositData, MigrateInstrData, MoveFundsData},
    state::{
        instrument::InstrAccountHeader,
        token::TokenState,
        types::{
            CappedI64,
            account_type::{INSTR, ROOT},
        },
    },
};
use solana_rpc_client::rpc_client::RpcClient;
use solana_sdk::{
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
};

use crate::{
    Helper,
    custom_sdk::traits::{BuildContext, Context},
    program_id,
};

pub struct CloseCandlesCtx {
    pub admin: Pubkey,
    pub root_account: Pubkey,
    pub candle_1m: Pubkey,
    pub candle_15m: Pubkey,
    pub candle_day: Pubkey,
}

pub struct CloseCandlesBuildCtx {
    pub admin: Pubkey,
    pub a_token_mint: Pubkey,
    pub b_token_mint: Pubkey,
}

impl BuildContext for CloseCandlesBuildCtx {}

pub const SPOT_DAY_CANDLES: u32 = 21;
pub const SPOT_15M_CANDLES: u32 = 20;
pub const SPOT_1M_CANDLES: u32 = 19;

impl Context for CloseCandlesCtx {
    type Build = CloseCandlesBuildCtx;

    fn build(
        rpc: &RpcClient,
        build_ctx: Self::Build,
    ) -> Result<Box<Self>, solana_rpc_client_api::client_error::AnyhowError> {
        let CloseCandlesBuildCtx {
            admin,
            a_token_mint,
            b_token_mint,
        } = build_ctx;

        let a_token_state_addr = a_token_mint.new_token_acc();

        let a_token_state = {
            let acc = rpc.get_account(&a_token_state_addr)?;
            unsafe { *(acc.data.as_ptr() as *const TokenState) }
        };

        let b_token_state_addr = b_token_mint.new_token_acc();

        let b_token_state = {
            let acc = rpc.get_account(&b_token_state_addr)?;
            unsafe { *(acc.data.as_ptr() as *const TokenState) }
        };

        Ok(Box::new(Self {
            admin,
            root_account: Pubkey::new_acc(ROOT),
            candle_1m: Pubkey::new_spot_acc(SPOT_1M_CANDLES, a_token_state.id, b_token_state.id),
            candle_15m: Pubkey::new_spot_acc(SPOT_15M_CANDLES, a_token_state.id, b_token_state.id),
            candle_day: Pubkey::new_spot_acc(SPOT_DAY_CANDLES, a_token_state.id, b_token_state.id),
        }))
    }

    fn create_instruction(&self) -> Vec<Instruction> {
        let accounts = vec![
            AccountMeta::new(self.admin, true),
            AccountMeta::new(self.root_account, false),
            AccountMeta::new(self.candle_1m, false),
            AccountMeta::new(self.candle_15m, false),
            AccountMeta::new(self.candle_day, false),
        ];

        vec![Instruction::new_with_bytes(
            program_id::ID,
            &[MigrateTokenInstruction::INSTRUCTION_NUMBER],
            accounts,
        )]
    }
}
