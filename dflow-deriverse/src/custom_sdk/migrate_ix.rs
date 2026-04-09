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

use solana_system_interface::program;
use spl_associated_token_account::get_associated_token_address_with_program_id;

use crate::{
    Helper,
    custom_sdk::traits::{BuildContext, Context},
    helper::{CappedNumber, get_dec_factor},
    program_id::{self, VERSION},
};

pub struct TokenCtx {
    mint: Pubkey,
    token_state: TokenState,
    token_state_addr: Pubkey,
    token_progra_id: Pubkey,
    new_vault_addr: Pubkey,
    old_vault_addr: Pubkey,
}

pub struct MigrateCtx {
    pub admin: Pubkey,
    pub root_account: Pubkey,
    pub a_ctx: TokenCtx,
    pub b_ctx: TokenCtx,
    pub lut_acc: Pubkey,
    pub instr_header_addr: Pubkey,
    pub instr_header: Box<InstrAccountHeader>,
    pub drvs_auth: Pubkey,
}

pub struct MigrateBuildCtx {
    pub admin: Pubkey,
    pub a_token_mint: Pubkey,
    pub b_token_mint: Pubkey,
}

impl BuildContext for MigrateBuildCtx {}

impl Context for MigrateCtx {
    type Build = MigrateBuildCtx;

    fn build(
        rpc: &RpcClient,
        build_ctx: Self::Build,
    ) -> Result<Box<Self>, solana_rpc_client_api::client_error::AnyhowError> {
        let MigrateBuildCtx {
            admin,
            a_token_mint,
            b_token_mint,
        } = build_ctx;

        let get_token_ctx =
            |token_mint| -> Result<TokenCtx, solana_rpc_client_api::client_error::AnyhowError> {
                let mint_acc = rpc.get_account(&token_mint)?;

                let token_state_addr = token_mint.new_token_acc();

                let token_state = {
                    let acc = rpc.get_account(&token_state_addr)?;
                    unsafe { *(acc.data.as_ptr() as *const TokenState) }
                };

                let old_vault = token_state.program_address;

                let (new_vault, _) = Pubkey::find_program_address(
                    &[token_mint.as_ref(), &VERSION.to_le_bytes()],
                    &program_id::id(),
                );

                Ok(TokenCtx {
                    mint: token_mint,
                    token_state: token_state,
                    token_state_addr,
                    token_progra_id: mint_acc.owner,
                    new_vault_addr: new_vault,
                    old_vault_addr: old_vault,
                })
            };

        let a_ctx = get_token_ctx(a_token_mint)?;
        let b_ctx = get_token_ctx(b_token_mint)?;

        let instr_addr = Pubkey::new_spot_acc(INSTR, a_ctx.token_state.id, b_ctx.token_state.id);

        let instr_state = {
            let acc = rpc.get_account(&instr_addr)?;
            unsafe { *(acc.data.as_ptr() as *const InstrAccountHeader) }
        };

        let (drvs_auth, _) = Pubkey::find_program_address(&[DRVS_SEED], &program_id::id());

        Ok(Box::new(Self {
            admin,
            root_account: Pubkey::new_acc(ROOT),
            a_ctx,
            b_ctx,
            lut_acc: instr_state.lut_address,
            instr_header: Box::new(instr_state),
            drvs_auth,
            instr_header_addr: instr_addr,
        }))
    }

    fn create_instruction(&self) -> Vec<Instruction> {
        let create_migrate_token_ix = |ctx: &TokenCtx| -> Instruction {
            let accounts = vec![
                AccountMeta::new(self.admin, true),
                AccountMeta::new(self.root_account, false),
                AccountMeta::new_readonly(ctx.mint, false),
                AccountMeta::new(ctx.new_vault_addr, false),
                AccountMeta::new(ctx.token_state_addr, false),
                AccountMeta::new_readonly(ctx.token_progra_id, false),
                AccountMeta::new_readonly(solana_system_interface::program::id(), false),
            ];
            Instruction::new_with_bytes(
                program_id::ID,
                &[MigrateTokenInstruction::INSTRUCTION_NUMBER],
                accounts,
            )
        };

        let migrate_instr_ix = {
            let accounts = vec![
                AccountMeta::new(self.admin, true),
                AccountMeta::new(self.root_account, false),
                AccountMeta::new(self.instr_header_addr, false),
                AccountMeta::new(self.lut_acc, false),
                AccountMeta::new(self.drvs_auth, false),
                AccountMeta::new(solana_address_lookup_table_interface::program::id(), false),
                AccountMeta::new(solana_system_interface::program::id(), false),
            ];

            Instruction::new_with_bytes(
                program_id::ID,
                bytes_of(&MigrateInstrData {
                    tag: MigrateInstrInstruction::INSTRUCTION_NUMBER,
                    instr_id: self.instr_header.instr_id,
                    ..Zeroable::zeroed()
                }),
                accounts,
            )
        };

        let move_funds_ix = |ctx: &TokenCtx| {
            let accounts = vec![
                AccountMeta::new(self.admin, true),
                AccountMeta::new(self.root_account, false),
                AccountMeta::new_readonly(ctx.mint, false),
                AccountMeta::new(ctx.new_vault_addr, false),
                AccountMeta::new(ctx.old_vault_addr, false),
                AccountMeta::new(ctx.token_state_addr, false),
                AccountMeta::new(self.drvs_auth, false),
                AccountMeta::new(ctx.token_progra_id, false),
            ];

            Instruction::new_with_bytes(
                program_id::ID,
                bytes_of(&MoveFundsData {
                    tag: MoveFundsInstruction::INSTRUCTION_NUMBER,
                    token_id: ctx.token_state.id,
                    amount: 0,
                    ..Zeroable::zeroed()
                }),
                accounts,
            )
        };

        vec![
            create_migrate_token_ix(&self.a_ctx),
            create_migrate_token_ix(&self.b_ctx),
            migrate_instr_ix,
            move_funds_ix(&self.a_ctx),
            move_funds_ix(&self.b_ctx),
        ]
    }
}
