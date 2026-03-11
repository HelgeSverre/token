(* Standard ML Syntax Highlighting Test *)
(* A purely functional expression evaluator with modules and functors. *)

(* ============================================================
   Signatures (module interfaces)
   ============================================================ *)

signature ORDERED =
sig
  type t
  val compare : t * t -> order
end

signature DICT =
sig
  type key
  type 'a dict
  val empty    : 'a dict
  val insert   : key * 'a * 'a dict -> 'a dict
  val lookup   : key * 'a dict -> 'a option
  val remove   : key * 'a dict -> 'a dict
  val toList   : 'a dict -> (key * 'a) list
  val fromList : (key * 'a) list -> 'a dict
  val size     : 'a dict -> int
  val map      : ('a -> 'b) -> 'a dict -> 'b dict
  val fold     : (key * 'a * 'b -> 'b) -> 'b -> 'a dict -> 'b
end

(* ============================================================
   Functor: Dictionary parameterized by key type
   ============================================================ *)

functor TreeDict(Key : ORDERED) :> DICT where type key = Key.t =
struct
  type key = Key.t

  datatype 'a tree =
      Leaf
    | Node of 'a tree * (key * 'a) * 'a tree * int

  type 'a dict = 'a tree

  val empty = Leaf

  fun height Leaf = 0
    | height (Node (_, _, _, h)) = h

  fun mkNode (l, kv, r) =
    Node (l, kv, r, 1 + Int.max (height l, height r))

  fun balance (l, kv, r) =
    let
      val hl = height l
      val hr = height r
    in
      if hl > hr + 1 then
        case l of
          Node (ll, lkv, lr, _) =>
            if height ll >= height lr
            then mkNode (ll, lkv, mkNode (lr, kv, r))
            else case lr of
              Node (lrl, lrkv, lrr, _) =>
                mkNode (mkNode (ll, lkv, lrl), lrkv, mkNode (lrr, kv, r))
            | _ => mkNode (l, kv, r)
        | _ => mkNode (l, kv, r)
      else if hr > hl + 1 then
        case r of
          Node (rl, rkv, rr, _) =>
            if height rr >= height rl
            then mkNode (mkNode (l, kv, rl), rkv, rr)
            else case rl of
              Node (rll, rlkv, rlr, _) =>
                mkNode (mkNode (l, kv, rll), rlkv, mkNode (rlr, rkv, rr))
            | _ => mkNode (l, kv, r)
        | _ => mkNode (l, kv, r)
      else
        mkNode (l, kv, r)
    end

  fun insert (k, v, Leaf) = Node (Leaf, (k, v), Leaf, 1)
    | insert (k, v, Node (l, (k', v'), r, h)) =
        case Key.compare (k, k') of
          LESS    => balance (insert (k, v, l), (k', v'), r)
        | GREATER => balance (l, (k', v'), insert (k, v, r))
        | EQUAL   => Node (l, (k, v), r, h)

  fun lookup (_, Leaf) = NONE
    | lookup (k, Node (l, (k', v), r, _)) =
        case Key.compare (k, k') of
          LESS    => lookup (k, l)
        | GREATER => lookup (k, r)
        | EQUAL   => SOME v

  fun removeMin (Node (Leaf, kv, r, _)) = (kv, r)
    | removeMin (Node (l, kv, r, _)) =
        let val (min, l') = removeMin l
        in (min, balance (l', kv, r)) end
    | removeMin Leaf = raise Fail "removeMin on empty"

  fun remove (_, Leaf) = Leaf
    | remove (k, Node (l, (k', v), r, _)) =
        case Key.compare (k, k') of
          LESS    => balance (remove (k, l), (k', v), r)
        | GREATER => balance (l, (k', v), remove (k, r))
        | EQUAL   =>
            case r of
              Leaf => l
            | _ =>
                let val (min, r') = removeMin r
                in balance (l, min, r') end

  fun toList Leaf = []
    | toList (Node (l, kv, r, _)) =
        toList l @ [kv] @ toList r

  fun fromList kvs = foldl (fn ((k, v), d) => insert (k, v, d)) empty kvs

  fun size Leaf = 0
    | size (Node (l, _, r, _)) = 1 + size l + size r

  fun map _ Leaf = Leaf
    | map f (Node (l, (k, v), r, h)) =
        Node (map f l, (k, f v), map f r, h)

  fun fold _ acc Leaf = acc
    | fold f acc (Node (l, (k, v), r, _)) =
        fold f (f (k, v, fold f acc l)) r
end

(* ============================================================
   Expression language
   ============================================================ *)

structure StringOrd : ORDERED =
struct
  type t = string
  val compare = String.compare
end

structure Env = TreeDict(StringOrd)

datatype expr =
    Num of real
  | Var of string
  | BinOp of binop * expr * expr
  | UnOp of unop * expr
  | Let of string * expr * expr
  | If of expr * expr * expr
  | Fun of string * expr
  | App of expr * expr

and binop = Add | Sub | Mul | Div | Mod | Pow | Eq | Lt | Gt
and unop = Neg | Abs | Not

datatype value =
    VNum of real
  | VBool of bool
  | VFun of string * expr * value Env.dict
  | VUnit

exception TypeError of string
exception DivByZero
exception UnboundVar of string

(* ============================================================
   Evaluator
   ============================================================ *)

fun eval (env : value Env.dict) (expr : expr) : value =
  case expr of
    Num n => VNum n
  | Var name =>
      (case Env.lookup (name, env) of
        SOME v => v
      | NONE => raise UnboundVar name)
  | BinOp (oper, e1, e2) =>
      let
        val v1 = eval env e1
        val v2 = eval env e2
      in
        evalBinOp (oper, v1, v2)
      end
  | UnOp (oper, e) =>
      evalUnOp (oper, eval env e)
  | Let (name, valueExpr, body) =>
      let val v = eval env valueExpr
      in eval (Env.insert (name, v, env)) body end
  | If (cond, thenE, elseE) =>
      (case eval env cond of
        VBool true  => eval env thenE
      | VBool false => eval env elseE
      | _ => raise TypeError "if condition must be boolean")
  | Fun (param, body) =>
      VFun (param, body, env)
  | App (funExpr, argExpr) =>
      (case eval env funExpr of
        VFun (param, body, closureEnv) =>
          let val argVal = eval env argExpr
          in eval (Env.insert (param, argVal, closureEnv)) body end
      | _ => raise TypeError "cannot apply non-function")

and evalBinOp (oper, VNum a, VNum b) =
    (case oper of
      Add => VNum (a + b)
    | Sub => VNum (a - b)
    | Mul => VNum (a * b)
    | Div => if Real.== (b, 0.0) then raise DivByZero
             else VNum (a / b)
    | Mod => VNum (Real.rem (a, b))
    | Pow => VNum (Math.pow (a, b))
    | Eq  => VBool (Real.== (a, b))
    | Lt  => VBool (a < b)
    | Gt  => VBool (a > b))
  | evalBinOp _ = raise TypeError "type mismatch in binary operation"

and evalUnOp (Neg, VNum n) = VNum (~n)
  | evalUnOp (Abs, VNum n) = VNum (Real.abs n)
  | evalUnOp (Not, VBool b) = VBool (not b)
  | evalUnOp _ = raise TypeError "type mismatch in unary operation"

(* ============================================================
   Pretty printer
   ============================================================ *)

fun valueToString (VNum n) =
      if Real.== (Real.realMod n, 0.0)
      then Int.toString (Real.round n)
      else Real.toString n
  | valueToString (VBool b) = Bool.toString b
  | valueToString (VFun (p, _, _)) = "<fun " ^ p ^ ">"
  | valueToString VUnit = "()"

(* ============================================================
   Main
   ============================================================ *)

val () =
  let
    (* let double = fn x -> x * 2 in double(21) *)
    val program =
      Let ("double",
        Fun ("x", BinOp (Mul, Var "x", Num 2.0)),
        Let ("square",
          Fun ("n", BinOp (Mul, Var "n", Var "n")),
          Let ("result",
            App (Var "square", App (Var "double", Num 5.0)),
            If (BinOp (Gt, Var "result", Num 50.0),
              Var "result",
              Num 0.0))))

    val result = eval Env.empty program
  in
    print ("Result: " ^ valueToString result ^ "\n")
  end
