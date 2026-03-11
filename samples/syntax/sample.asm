; x86-64 Assembly Syntax Highlighting Test (NASM syntax)
; A string processing library with system calls and SIMD.

section .data
    ; String constants
    hello_msg:  db "Task Manager v1.0", 10, 0
    hello_len:  equ $ - hello_msg

    newline:    db 10
    separator:  db "========================", 10, 0
    sep_len:    equ $ - separator

    fmt_task:   db "#%d [%s] %s", 10, 0
    fmt_stats:  db "Total: %d, Done: %d (%.1f%%)", 10, 0

    ; Status icons (null-terminated)
    icon_open:      db "[ ]", 0
    icon_progress:  db "[~]", 0
    icon_done:      db "[x]", 0
    icon_cancelled: db "[-]", 0

    ; Status icon lookup table
    status_icons:
        dq icon_open
        dq icon_progress
        dq icon_done
        dq icon_cancelled

    ; Priority labels
    prio_low:       db "low", 0
    prio_medium:    db "medium", 0
    prio_high:      db "high", 0
    prio_critical:  db "critical", 0

    priority_labels:
        dq prio_low
        dq prio_medium
        dq prio_high
        dq prio_critical

    ; Sample task data
    task1_title: db "Implement syntax highlighting", 0
    task2_title: db "Fix cursor blinking", 0
    task3_title: db "Add split view", 0
    task4_title: db "Write documentation", 0
    task5_title: db "Performance profiling", 0

    ; Hex lookup table for fast conversion
    hex_chars: db "0123456789abcdef"

section .bss
    ; Task structure (SOA layout)
    ; struct Task { u32 id, u8 status, u8 priority, char* title }
    task_ids:       resd 256     ; 256 task IDs
    task_statuses:  resb 256     ; 256 status bytes
    task_priorities:resb 256     ; 256 priority bytes
    task_titles:    resq 256     ; 256 title pointers
    task_count:     resd 1       ; number of tasks

    ; String buffer for formatting
    str_buffer:     resb 1024
    int_buffer:     resb 32

    ; Statistics
    stat_total:     resd 1
    stat_done:      resd 1

section .text
    global _start
    extern printf, exit

; ============================================================
; System call wrappers
; ============================================================

; Write string to stdout
; rdi = pointer to string, rsi = length
sys_write:
    push rdi
    push rsi
    mov rdx, rsi        ; length
    mov rsi, rdi         ; buffer
    mov rdi, 1           ; stdout
    mov rax, 1           ; sys_write
    syscall
    pop rsi
    pop rdi
    ret

; Exit with status code in rdi
sys_exit:
    mov rax, 60          ; sys_exit
    syscall

; ============================================================
; String operations
; ============================================================

; Calculate string length (null-terminated)
; rdi = string pointer
; Returns length in rax
strlen:
    xor rax, rax         ; counter = 0
    mov rcx, rdi         ; save start
.loop:
    cmp byte [rdi + rax], 0
    je .done
    inc rax
    jmp .loop
.done:
    ret

; Copy string src to dst
; rdi = dst, rsi = src
; Returns bytes copied in rax
strcpy:
    xor rax, rax
.loop:
    mov cl, [rsi + rax]
    mov [rdi + rax], cl
    test cl, cl
    jz .done
    inc rax
    jmp .loop
.done:
    ret

; Compare two strings
; rdi = str1, rsi = str2
; Returns 0 if equal, -1 if less, 1 if greater
strcmp:
    xor rcx, rcx
.loop:
    mov al, [rdi + rcx]
    mov bl, [rsi + rcx]
    cmp al, bl
    jl .less
    jg .greater
    test al, al
    jz .equal
    inc rcx
    jmp .loop
.equal:
    xor eax, eax
    ret
.less:
    mov eax, -1
    ret
.greater:
    mov eax, 1
    ret

; ============================================================
; Integer to string conversion
; ============================================================

; Convert unsigned integer to decimal string
; rdi = number, rsi = buffer
; Returns string length in rax
itoa:
    push rbx
    push rcx
    push rdx

    mov rax, rdi         ; number
    mov rbx, rsi         ; buffer start
    mov rcx, rsi         ; current position

    ; Handle zero
    test rax, rax
    jnz .convert
    mov byte [rcx], '0'
    inc rcx
    jmp .null_term

.convert:
    ; Push digits in reverse order
    xor rdx, rdx
    push rax             ; sentinel
.div_loop:
    test rax, rax
    jz .reverse
    mov rdx, 0
    mov r8, 10
    div r8               ; rax = quotient, rdx = remainder
    add dl, '0'
    push rdx
    jmp .div_loop

.reverse:
    pop rax              ; pop sentinel or digit
    cmp rax, rdi         ; check if original number (sentinel)
    je .null_term
    mov [rcx], al
    inc rcx
    jmp .reverse

.null_term:
    mov byte [rcx], 0
    mov rax, rcx
    sub rax, rbx         ; length

    pop rdx
    pop rcx
    pop rbx
    ret

; ============================================================
; SIMD: Fast character counting (count occurrences of a byte)
; ============================================================

; Count occurrences of byte in string using SSE2
; rdi = string, rsi = string length, dl = byte to count
; Returns count in rax
count_byte_simd:
    xor rax, rax         ; total count
    xor rcx, rcx         ; position

    ; Broadcast search byte to all lanes of xmm0
    movd xmm0, edx
    punpcklbw xmm0, xmm0
    punpcklwd xmm0, xmm0
    pshufd xmm0, xmm0, 0

    ; Process 16 bytes at a time
.simd_loop:
    lea r8, [rcx + 16]
    cmp r8, rsi
    jg .scalar_loop      ; not enough bytes for SIMD

    movdqu xmm1, [rdi + rcx]
    pcmpeqb xmm1, xmm0  ; compare bytes
    pmovmskb edx, xmm1   ; extract mask
    popcnt edx, edx       ; count set bits
    add eax, edx
    add rcx, 16
    jmp .simd_loop

    ; Process remaining bytes one at a time
.scalar_loop:
    cmp rcx, rsi
    jge .done
    cmp byte [rdi + rcx], dl
    jne .skip
    inc eax
.skip:
    inc rcx
    jmp .scalar_loop
.done:
    ret

; ============================================================
; Task management
; ============================================================

; Add a task
; rdi = title pointer, sil = priority (0-3)
; Returns task ID in eax
add_task:
    push rbx

    ; Get current count
    mov eax, [task_count]
    cmp eax, 256
    jge .full

    mov ebx, eax         ; index

    ; Set ID (1-based)
    lea ecx, [eax + 1]
    mov [task_ids + rbx * 4], ecx

    ; Set status (open = 0)
    mov byte [task_statuses + rbx], 0

    ; Set priority
    mov [task_priorities + rbx], sil

    ; Set title pointer
    mov [task_titles + rbx * 8], rdi

    ; Increment count
    inc dword [task_count]

    ; Return ID
    mov eax, ecx
    pop rbx
    ret

.full:
    xor eax, eax         ; return 0 on failure
    pop rbx
    ret

; Print a single task
; edi = task index
print_task:
    push rbx
    push r12

    mov ebx, edi         ; save index

    ; Print "  #"
    lea rdi, [rel str_buffer]
    mov byte [rdi], ' '
    mov byte [rdi + 1], ' '
    mov byte [rdi + 2], '#'

    ; Convert ID to string
    mov edi, [task_ids + rbx * 4]
    lea rsi, [rel str_buffer + 3]
    call itoa
    mov r12, rax         ; save length

    ; Add space
    lea rdi, [rel str_buffer + 3]
    add rdi, r12
    mov byte [rdi], ' '
    inc rdi

    ; Add status icon
    movzx eax, byte [task_statuses + rbx]
    mov rsi, [status_icons + rax * 8]
    call strcpy
    add rdi, rax
    mov byte [rdi], ' '
    inc rdi

    ; Add title
    mov rsi, [task_titles + rbx * 8]
    call strcpy
    add rdi, rax

    ; Add newline
    mov byte [rdi], 10
    inc rdi
    mov byte [rdi], 0

    ; Write it all
    lea rdi, [rel str_buffer]
    lea rsi, [rdi]
    call strlen
    mov rsi, rax
    call sys_write

    pop r12
    pop rbx
    ret

; ============================================================
; Entry point
; ============================================================

_start:
    ; Print banner
    lea rdi, [rel hello_msg]
    mov rsi, hello_len
    call sys_write

    lea rdi, [rel separator]
    mov rsi, sep_len
    call sys_write

    ; Add tasks
    lea rdi, [rel task1_title]
    mov sil, 2                   ; high priority
    call add_task

    lea rdi, [rel task2_title]
    mov sil, 0                   ; low priority
    call add_task

    lea rdi, [rel task3_title]
    mov sil, 1                   ; medium priority
    call add_task

    lea rdi, [rel task4_title]
    mov sil, 1                   ; medium
    call add_task

    lea rdi, [rel task5_title]
    mov sil, 2                   ; high
    call add_task

    ; Mark task 2 as done (status = 2)
    mov byte [task_statuses + 1], 2

    ; Mark task 1 as in progress (status = 1)
    mov byte [task_statuses + 0], 1

    ; Print all tasks
    xor ebx, ebx
.print_loop:
    cmp ebx, [task_count]
    jge .print_done
    mov edi, ebx
    call print_task
    inc ebx
    jmp .print_loop
.print_done:

    ; Exit
    xor edi, edi
    call sys_exit
