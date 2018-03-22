// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

use ast_factory::*;
use errors::Result as LocalResult;
use uuid::Uuid;

const RETURN_LABEL: &str = "return";

pub struct CfgMethod<'a: 'b, 'b> {
    ast_factory: &'b AstFactory<'a>,
    uuid: Uuid,
    method_name: String,
    formal_args: Vec<LocalVarDecl<'a>>,
    formal_returns: Vec<LocalVarDecl<'a>>,
    local_vars: Vec<LocalVarDecl<'a>>,
    basic_blocks: Vec<CfgBlock<'a>>,
    basic_blocks_labels: Vec<String>,
}

#[derive(Clone)]
struct CfgBlock<'a> {
    invs: Vec<Expr<'a>>,
    stmt: Stmt<'a>,
    successor: Successor<'a>,
}

#[derive(Clone)]
pub enum Successor<'a> {
    Unreachable(),
    Return(),
    Goto(CfgBlockIndex),
    GotoSwitch(Vec<(Expr<'a>, CfgBlockIndex)>, CfgBlockIndex),
    GotoIf(Expr<'a>, CfgBlockIndex, CfgBlockIndex),
}

#[derive(Clone, Copy)]
pub struct CfgBlockIndex {
    method_uuid: Uuid,
    block_index: usize,
}

impl<'a: 'b, 'b> CfgMethod<'a, 'b> {
    pub fn new(
        ast_factory: &'b AstFactory<'a>,
        method_name: String,
        formal_args: Vec<LocalVarDecl<'a>>,
        formal_returns: Vec<LocalVarDecl<'a>>,
        local_vars: Vec<LocalVarDecl<'a>>,
    ) -> Self {
        CfgMethod {
            ast_factory,
            uuid: Uuid::new_v4(),
            method_name: method_name,
            formal_args,
            formal_returns,
            local_vars,
            basic_blocks: vec![],
            basic_blocks_labels: vec![],
        }
    }

    pub fn add_block(&mut self, label: &str, invs: Vec<Expr<'a>>, stmt: Stmt<'a>) -> CfgBlockIndex {
        assert!(label.chars().take(1).all(|c| c.is_alphabetic() || c == '_'));
        assert!(label.chars().skip(1).all(|c| c.is_alphanumeric() || c == '_'));
        assert!(self.basic_blocks_labels.iter().all(|l| l != label));
        assert!(label != RETURN_LABEL);
        let index = self.basic_blocks.len();
        self.basic_blocks_labels.push(label.to_string());
        self.basic_blocks.push(CfgBlock {
            invs,
            stmt,
            successor: Successor::Unreachable(),
        });
        CfgBlockIndex {
            method_uuid: self.uuid,
            block_index: index,
        }
    }

    pub fn set_successor(&mut self, index: CfgBlockIndex, successor: Successor<'a>) {
        assert_eq!(
            self.uuid, index.method_uuid,
            "The provided CfgBlockIndex doesn't belong to this CfgMethod"
        );
        self.basic_blocks[index.block_index].successor = successor;
    }

    #[cfg_attr(feature = "cargo-clippy", allow(wrong_self_convention))]
    pub fn to_ast(self) -> LocalResult<Method<'a>> {
        let mut blocks_ast: Vec<Stmt> = vec![];
        let mut declarations: Vec<Declaration> = vec![];

        for &local_var in &self.local_vars {
            declarations.push(local_var.into());
        }

        for (index, block) in self.basic_blocks.iter().enumerate() {
            blocks_ast.push(block_to_ast(
                self.ast_factory,
                &self.basic_blocks_labels,
                block,
                index,
            ));
            declarations.push(
                self.ast_factory
                    .label(&index_to_label(&self.basic_blocks_labels, index), &[])
                    .into(),
            );
        }
        blocks_ast.push(
            self.ast_factory
                .label(&return_label(), &[]),
        );
        declarations.push(
            self.ast_factory
                .label(&return_label(), &[])
                .into(),
        );

        let method_body = Some(self.ast_factory.seqn(&blocks_ast, &declarations));

        let method = self.ast_factory.method(
            &self.method_name,
            &self.formal_args,
            &self.formal_returns,
            &[],
            &[],
            method_body,
        );

        Ok(method)
    }
}

fn index_to_label(basic_block_labels: &Vec<String>, index: usize) -> String {
    basic_block_labels[index].clone()
}

fn return_label() -> String {
    RETURN_LABEL.to_string()
}

fn successor_to_ast<'a>(
    ast: &'a AstFactory,
    basic_block_labels: &Vec<String>,
    successor: &Successor<'a>,
) -> Stmt<'a> {
    match *successor {
        Successor::Unreachable() => ast.assert(ast.false_lit(), ast.no_position()),
        Successor::Return() => ast.goto(&return_label()),
        Successor::Goto(target) => ast.goto(&index_to_label(basic_block_labels, target.block_index)),
        Successor::GotoSwitch(ref successors, ref default_target) => {
            let skip = ast.seqn(&[], &[]);
            let mut stmts: Vec<Stmt> = vec![];
            for &(test, target) in successors {
                let goto = ast.goto(&index_to_label(basic_block_labels, target.block_index));
                let conditional_goto = ast.if_stmt(test, goto, skip);
                stmts.push(conditional_goto);
            }
            let default_goto = ast.goto(&index_to_label(basic_block_labels, default_target.block_index));
            stmts.push(default_goto);
            ast.seqn(&stmts, &[])
        }
        Successor::GotoIf(test, then_target, else_target) => {
            let then_goto = ast.goto(&index_to_label(basic_block_labels, then_target.block_index));
            let else_goto = ast.goto(&index_to_label(basic_block_labels, else_target.block_index));
            ast.if_stmt(test, then_goto, else_goto)
        }
    }
}

fn block_to_ast<'a>(
    ast: &'a AstFactory,
    basic_block_labels: &Vec<String>,
    block: &CfgBlock<'a>,
    index: usize,
) -> Stmt<'a> {
    let label = index_to_label(basic_block_labels, index);
    ast.seqn(
        &[
            ast.label(&label, &block.invs),
            block.stmt,
            successor_to_ast(ast, basic_block_labels, &block.successor),
        ],
        &[],
    )
}
