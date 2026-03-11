(** OCaml Syntax Highlighting Test
    A simple interpreter for a calculator language with variables. *)

open Printf

(* ============================================================
   AST and Types
   ============================================================ *)

type binop = Add | Sub | Mul | Div | Mod | Pow
type unop = Neg | Abs
type comparison = Eq | Ne | Lt | Le | Gt | Ge

type expr =
  | Lit of float
  | Var of string
  | BinOp of binop * expr * expr
  | UnOp of unop * expr
  | Let of string * expr * expr
  | If of expr * comparison * expr * expr * expr
  | Fun of string * expr
  | App of expr * expr
  | Seq of expr list

type value =
  | VNum of float
  | VFun of string * expr * env
  | VUnit

and env = (string * value) list

(* ============================================================
   Environment
   ============================================================ *)

let rec lookup name = function
  | [] -> failwith (sprintf "Unbound variable: %s" name)
  | (k, v) :: _ when k = name -> v
  | _ :: rest -> lookup name rest

let extend name value env = (name, value) :: env

(* ============================================================
   Pretty printing
   ============================================================ *)

let string_of_binop = function
  | Add -> "+" | Sub -> "-" | Mul -> "*"
  | Div -> "/" | Mod -> "%" | Pow -> "**"

let string_of_unop = function
  | Neg -> "-" | Abs -> "abs"

let string_of_comparison = function
  | Eq -> "==" | Ne -> "!=" | Lt -> "<"
  | Le -> "<=" | Gt -> ">" | Ge -> ">="

let rec string_of_expr = function
  | Lit f ->
    if Float.is_integer f then sprintf "%.0f" f
    else sprintf "%g" f
  | Var name -> name
  | BinOp (op, lhs, rhs) ->
    sprintf "(%s %s %s)"
      (string_of_expr lhs) (string_of_binop op) (string_of_expr rhs)
  | UnOp (op, e) ->
    sprintf "(%s %s)" (string_of_unop op) (string_of_expr e)
  | Let (name, value, body) ->
    sprintf "(let %s = %s in %s)"
      name (string_of_expr value) (string_of_expr body)
  | If (lhs, cmp, rhs, then_e, else_e) ->
    sprintf "(if %s %s %s then %s else %s)"
      (string_of_expr lhs) (string_of_comparison cmp) (string_of_expr rhs)
      (string_of_expr then_e) (string_of_expr else_e)
  | Fun (param, body) ->
    sprintf "(fun %s -> %s)" param (string_of_expr body)
  | App (fn, arg) ->
    sprintf "(%s %s)" (string_of_expr fn) (string_of_expr arg)
  | Seq exprs ->
    exprs |> List.map string_of_expr |> String.concat "; "
    |> sprintf "(%s)"

let string_of_value = function
  | VNum f ->
    if Float.is_integer f then sprintf "%.0f" f
    else sprintf "%g" f
  | VFun (param, _, _) -> sprintf "<fun %s>" param
  | VUnit -> "()"

(* ============================================================
   Evaluator
   ============================================================ *)

exception DivisionByZero
exception TypeError of string

let as_num = function
  | VNum f -> f
  | v -> raise (TypeError (sprintf "Expected number, got %s" (string_of_value v)))

let eval_binop op a b =
  let a = as_num a and b = as_num b in
  match op with
  | Add -> VNum (a +. b)
  | Sub -> VNum (a -. b)
  | Mul -> VNum (a *. b)
  | Div ->
    if b = 0.0 then raise DivisionByZero
    else VNum (a /. b)
  | Mod ->
    if b = 0.0 then raise DivisionByZero
    else VNum (Float.rem a b)
  | Pow -> VNum (a ** b)

let eval_unop op v =
  let n = as_num v in
  match op with
  | Neg -> VNum (-.n)
  | Abs -> VNum (Float.abs n)

let eval_comparison cmp a b =
  let a = as_num a and b = as_num b in
  match cmp with
  | Eq -> a = b  | Ne -> a <> b | Lt -> a < b
  | Le -> a <= b | Gt -> a > b  | Ge -> a >= b

let rec eval (env : env) (expr : expr) : value =
  match expr with
  | Lit f -> VNum f
  | Var name -> lookup name env
  | BinOp (op, lhs, rhs) ->
    let a = eval env lhs in
    let b = eval env rhs in
    eval_binop op a b
  | UnOp (op, e) ->
    eval_unop op (eval env e)
  | Let (name, value_expr, body) ->
    let v = eval env value_expr in
    eval (extend name v env) body
  | If (lhs, cmp, rhs, then_expr, else_expr) ->
    let a = eval env lhs in
    let b = eval env rhs in
    if eval_comparison cmp a b then eval env then_expr
    else eval env else_expr
  | Fun (param, body) ->
    VFun (param, body, env)
  | App (fn_expr, arg_expr) ->
    let fn_val = eval env fn_expr in
    let arg_val = eval env arg_expr in
    begin match fn_val with
      | VFun (param, body, closure_env) ->
        eval (extend param arg_val closure_env) body
      | _ -> raise (TypeError "Cannot apply non-function")
    end
  | Seq [] -> VUnit
  | Seq [e] -> eval env e
  | Seq (e :: rest) ->
    let _ = eval env e in
    eval env (Seq rest)

(* ============================================================
   Standard library
   ============================================================ *)

let builtins : env =
  let math_const name value = (name, VNum value) in
  [
    math_const "pi" Float.pi;
    math_const "e" (exp 1.0);
    math_const "inf" Float.infinity;
    math_const "nan" Float.nan;
  ]

(* ============================================================
   Module for serialization
   ============================================================ *)

module Serialize = struct
  type format = Json | Sexp | Pretty

  let to_json value =
    let buf = Buffer.create 64 in
    let rec go = function
      | VNum f ->
        Buffer.add_string buf (sprintf "%g" f)
      | VFun (param, _, _) ->
        Buffer.add_string buf (sprintf "{\"type\":\"function\",\"param\":\"%s\"}" param)
      | VUnit ->
        Buffer.add_string buf "null"
    in
    go value;
    Buffer.contents buf

  let to_sexp value =
    let rec go = function
      | VNum f -> sprintf "%g" f
      | VFun (param, body, _) ->
        sprintf "(lambda (%s) %s)" param (string_of_expr body)
      | VUnit -> "()"
    in
    go value

  let format_value ~format value =
    match format with
    | Json -> to_json value
    | Sexp -> to_sexp value
    | Pretty -> string_of_value value
end

(* ============================================================
   Tests
   ============================================================ *)

let%test "basic arithmetic" =
  let expr = BinOp (Add, Lit 2.0, BinOp (Mul, Lit 3.0, Lit 4.0)) in
  eval builtins expr = VNum 14.0

let%test "let binding" =
  let expr = Let ("x", Lit 10.0, BinOp (Mul, Var "x", Var "x")) in
  eval builtins expr = VNum 100.0

let%test "higher-order function" =
  let double = Fun ("x", BinOp (Mul, Var "x", Lit 2.0)) in
  let expr = App (double, Lit 21.0) in
  eval builtins expr = VNum 42.0

let%test "conditional" =
  let expr = If (Lit 5.0, Gt, Lit 3.0, Lit 1.0, Lit 0.0) in
  eval builtins expr = VNum 1.0

(* ============================================================
   Main
   ============================================================ *)

let () =
  (* let double = fun x -> x * 2 in double(21) *)
  let program =
    Let ("double",
      Fun ("x", BinOp (Mul, Var "x", Lit 2.0)),
      Seq [
        App (Var "double", Lit 21.0);
        Let ("square",
          Fun ("n", BinOp (Mul, Var "n", Var "n")),
          App (Var "square", App (Var "double", Lit 5.0)))
      ])
  in
  let result = eval builtins program in
  printf "Result: %s\n" (string_of_value result);
  printf "JSON: %s\n" (Serialize.format_value ~format:Json result)
