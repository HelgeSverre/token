\ Forth Syntax Highlighting Test
\ A stack-based calculator with dictionary, conditionals, and memory.

\ ============================================================
\ Constants and variables
\ ============================================================

decimal
1000 constant MAX-STACK
  64 constant NAME-LEN
  16 constant MAX-VARS

variable total-ops
variable verbose-mode

0 total-ops !
0 verbose-mode !

\ ============================================================
\ Stack manipulation words
\ ============================================================

\ ( n -- n n ) Duplicate top of stack
\ DUP is built-in, but we can define helpers

: 2dup    ( a b -- a b a b )  over over ;
: 2drop   ( a b -- )  drop drop ;
: nip     ( a b -- b )  swap drop ;
: tuck    ( a b -- b a b )  swap over ;
: rot     ( a b c -- b c a )  >r swap r> swap ;
: -rot    ( a b c -- c a b )  rot rot ;

\ ============================================================
\ Arithmetic helpers
\ ============================================================

: square  ( n -- n^2 )  dup * ;
: cube    ( n -- n^3 )  dup dup * * ;
: abs     ( n -- |n| )  dup 0< if negate then ;
: max     ( a b -- max )  2dup < if nip else drop then ;
: min     ( a b -- min )  2dup > if nip else drop then ;
: clamp   ( n lo hi -- clamped )  rot min max ;
: between? ( n lo hi -- flag )  >r over > if r> drop drop false
                                 else r> <= then ;

\ Fixed-point arithmetic (16.16)
65536 constant FXSCALE

: f*  ( f1 f2 -- f1*f2 )  FXSCALE */ ;
: f/  ( f1 f2 -- f1/f2 )  FXSCALE swap */ ;
: >f  ( n -- fixed )  FXSCALE * ;
: f>  ( fixed -- n )  FXSCALE / ;
: f.  ( fixed -- )  dup abs FXSCALE /mod
      swap FXSCALE 1000 */ abs
      rot 0< if ." -" then
      . ." ." 0 <# # # # #> type ;

\ ============================================================
\ String handling
\ ============================================================

: counted-type  ( addr -- )
  count type ;

: string=  ( addr1 u1 addr2 u2 -- flag )
  rot over <> if 2drop drop false exit then
  0 do
    over i + c@ over i + c@ <> if
      2drop false unloop exit
    then
  loop
  2drop true ;

\ Print n copies of a character
: emit-n  ( char n -- )
  0 do dup emit loop drop ;

\ Print a horizontal line
: hline  ( width -- )
  [char] - swap emit-n cr ;

\ Right-justify a number in a field
: .r  ( n width -- )
  >r dup abs 0 <# #s rot sign #>
  r> over - spaces type ;

\ ============================================================
\ Array operations
\ ============================================================

\ Create a simple array
: array  ( n -- )
  create cells allot
  does> swap cells + ;

\ Initialize array to zeros
: array-clear  ( n addr -- )
  swap 0 do
    0 over i cells + !
  loop drop ;

\ Sum elements
: array-sum  ( n addr -- sum )
  0 -rot
  0 do
    dup i cells + @ rot + swap
  loop drop ;

\ ============================================================
\ Task data structure
\ ============================================================

\ Task fields (offsets)
0                constant TASK-ID
1 cells          constant TASK-STATUS    \ 0=open 1=progress 2=done 3=cancelled
2 cells          constant TASK-PRIORITY  \ 0=low 1=medium 2=high 3=critical
3 cells          constant TASK-TITLE     \ pointer to counted string
4 cells constant TASK-SIZE

\ Status names
create status-names
  ," open"
  ," in_progress"
  ," done"
  ," cancelled"

\ Priority names
create priority-names
  ," low"
  ," medium"
  ," high"
  ," critical"

\ Allocate task storage
10 constant MAX-TASKS
create tasks MAX-TASKS TASK-SIZE * allot
variable task-count
0 task-count !

\ Create a new task
: new-task  ( title-addr title-len priority -- id )
  task-count @ MAX-TASKS >= abort" Too many tasks"

  task-count @ TASK-SIZE * tasks +  ( title priority task-addr )
  >r
  task-count @ r@ TASK-ID + !      \ set id
  0 r@ TASK-STATUS + !             \ status = open
  r@ TASK-PRIORITY + !             \ set priority
  here r@ TASK-TITLE + !           \ store title pointer
  dup c, 0 do dup i + c@ c, loop drop  \ store counted string
  r> drop

  task-count @ dup 1+ task-count !  \ return id and increment
;

\ Get task field address
: task>  ( id field -- addr )
  swap TASK-SIZE * tasks + + ;

\ Print task status icon
: .status  ( status -- )
  case
    0 of ." [ ] " endof
    1 of ." [~] " endof
    2 of ." [x] " endof
    3 of ." [-] " endof
    ." [?] "
  endcase ;

\ Print a task
: .task  ( id -- )
  dup TASK-ID task> @ ." #" 3 .r space
  dup TASK-STATUS task> @ .status
  dup TASK-PRIORITY task> @ case
    0 of ."   " endof
    1 of ." ! " endof
    2 of ." !! " endof
    3 of ." !!!" endof
  endcase
  TASK-TITLE task> @ counted-type
  cr ;

\ Print all tasks
: .tasks  ( -- )
  cr ." === Tasks ===" cr
  40 hline
  task-count @ 0 do
    i .task
  loop
  40 hline ;

\ ============================================================
\ Statistics
\ ============================================================

: count-status  ( status -- count )
  0 swap
  task-count @ 0 do
    i TASK-STATUS task> @ over = if
      swap 1+ swap
    then
  loop drop ;

: .stats  ( -- )
  cr ." === Statistics ===" cr
  ." Total:       " task-count @ . cr
  ." Open:        " 0 count-status . cr
  ." In Progress: " 1 count-status . cr
  ." Done:        " 2 count-status . cr
  ." Cancelled:   " 3 count-status . cr
  cr
  ." Completion:  "
  task-count @ dup 0> if
    2 count-status 100 * swap /
    . ." %"
  else
    drop ." 0%"
  then cr ;

\ ============================================================
\ Interactive calculator (RPN)
\ ============================================================

: banner
  cr
  ." ╔═══════════════════════════╗" cr
  ." ║   Forth Calculator v1.0   ║" cr
  ." ║   Type 'help' for usage   ║" cr
  ." ╚═══════════════════════════╝" cr
  cr ;

: help
  ." Commands:" cr
  ."   <number>  Push number onto stack" cr
  ."   + - * /   Arithmetic operations" cr
  ."   .         Print top of stack" cr
  ."   .s        Print entire stack" cr
  ."   dup       Duplicate top" cr
  ."   swap      Swap top two" cr
  ."   drop      Remove top" cr
  ."   square    Square top value" cr
  ."   tasks     Show all tasks" cr
  ."   stats     Show statistics" cr
  ."   bye       Exit" cr ;

\ ============================================================
\ Main
\ ============================================================

: main
  banner

  \ Create sample tasks
  s" Implement syntax highlighting" 2 new-task drop
  s" Fix cursor blinking"          0 new-task drop
  s" Add split view"               1 new-task drop
  s" Write documentation"          1 new-task drop
  s" Performance profiling"        2 new-task drop

  \ Complete a task
  2 1 TASK-STATUS task> !  \ task 1: in_progress
  1 2 TASK-STATUS task> !  \ task 2: done

  .tasks
  .stats

  ." Ready." cr ;

main
