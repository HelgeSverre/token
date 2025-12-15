#!/bin/bash
# Bash Syntax Highlighting Test
# This file demonstrates various Bash syntax constructs.

# Exit on error, undefined variables, and pipe failures
set -euo pipefail

# Constants (readonly)
readonly VERSION="1.0.0"
readonly SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
readonly SCRIPT_NAME="$(basename "$0")"

# Configuration
declare -A CONFIG=(
    [debug]="false"
    [verbose]="false"
    [output_dir]="/tmp/output"
    [log_file]="/var/log/script.log"
)

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Logging functions
log_info() {
    echo -e "${GREEN}[INFO]${NC} $*"
}

log_warn() {
    echo -e "${YELLOW}[WARN]${NC} $*" >&2
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $*" >&2
}

log_debug() {
    if [[ "${CONFIG[debug]}" == "true" ]]; then
        echo -e "${BLUE}[DEBUG]${NC} $*"
    fi
}

# Print usage information
usage() {
    cat << EOF
Usage: $SCRIPT_NAME [OPTIONS] <command> [arguments]

Options:
    -h, --help          Show this help message
    -v, --verbose       Enable verbose output
    -d, --debug         Enable debug mode
    -o, --output DIR    Set output directory (default: ${CONFIG[output_dir]})
    --version           Show version information

Commands:
    process FILE        Process the given file
    analyze DIR         Analyze directory contents
    cleanup             Clean up temporary files

Examples:
    $SCRIPT_NAME -v process input.txt
    $SCRIPT_NAME --debug analyze /path/to/dir

EOF
}

# Version information
version() {
    echo "$SCRIPT_NAME version $VERSION"
}

# Parse command line arguments
parse_args() {
    local positional=()
    
    while [[ $# -gt 0 ]]; do
        case $1 in
            -h|--help)
                usage
                exit 0
                ;;
            -v|--verbose)
                CONFIG[verbose]="true"
                shift
                ;;
            -d|--debug)
                CONFIG[debug]="true"
                shift
                ;;
            -o|--output)
                if [[ -n "${2:-}" ]]; then
                    CONFIG[output_dir]="$2"
                    shift 2
                else
                    log_error "Option -o requires an argument"
                    exit 1
                fi
                ;;
            --version)
                version
                exit 0
                ;;
            --)
                shift
                positional+=("$@")
                break
                ;;
            -*)
                log_error "Unknown option: $1"
                usage
                exit 1
                ;;
            *)
                positional+=("$1")
                shift
                ;;
        esac
    done
    
    # Restore positional parameters
    set -- "${positional[@]}"
    ARGS=("$@")
}

# Check if a command exists
command_exists() {
    command -v "$1" &> /dev/null
}

# Check dependencies
check_dependencies() {
    local deps=("curl" "jq" "awk" "sed")
    local missing=()
    
    for dep in "${deps[@]}"; do
        if ! command_exists "$dep"; then
            missing+=("$dep")
        fi
    done
    
    if [[ ${#missing[@]} -gt 0 ]]; then
        log_error "Missing dependencies: ${missing[*]}"
        exit 1
    fi
}

# Create directory if it doesn't exist
ensure_dir() {
    local dir="$1"
    if [[ ! -d "$dir" ]]; then
        mkdir -p "$dir"
        log_debug "Created directory: $dir"
    fi
}

# Process a file
process_file() {
    local file="$1"
    
    if [[ ! -f "$file" ]]; then
        log_error "File not found: $file"
        return 1
    fi
    
    log_info "Processing file: $file"
    
    # Count lines, words, characters
    local lines words chars
    read -r lines words chars _ <<< "$(wc -lwc < "$file")"
    
    echo "Statistics for $file:"
    echo "  Lines: $lines"
    echo "  Words: $words"
    echo "  Characters: $chars"
    
    # Process each line
    local line_num=0
    while IFS= read -r line || [[ -n "$line" ]]; do
        ((line_num++))
        
        # Skip empty lines and comments
        [[ -z "$line" || "$line" =~ ^[[:space:]]*# ]] && continue
        
        if [[ "${CONFIG[verbose]}" == "true" ]]; then
            echo "  Line $line_num: $line"
        fi
    done < "$file"
    
    return 0
}

# Analyze a directory
analyze_dir() {
    local dir="$1"
    
    if [[ ! -d "$dir" ]]; then
        log_error "Directory not found: $dir"
        return 1
    fi
    
    log_info "Analyzing directory: $dir"
    
    # Count files by extension
    declare -A ext_counts
    
    while IFS= read -r -d '' file; do
        local ext="${file##*.}"
        if [[ "$ext" == "$file" ]]; then
            ext="(no extension)"
        fi
        ((ext_counts[$ext]++)) || ext_counts[$ext]=1
    done < <(find "$dir" -type f -print0)
    
    echo "Files by extension:"
    for ext in "${!ext_counts[@]}"; do
        printf "  %-20s %d\n" "$ext:" "${ext_counts[$ext]}"
    done | sort -t: -k2 -rn
    
    # Directory size
    local size
    size=$(du -sh "$dir" 2>/dev/null | cut -f1)
    echo "Total size: $size"
}

# Cleanup function
cleanup() {
    local exit_code=$?
    log_debug "Cleaning up..."
    
    # Remove temporary files
    if [[ -d "${TMPDIR:-/tmp}/$SCRIPT_NAME.$$" ]]; then
        rm -rf "${TMPDIR:-/tmp}/$SCRIPT_NAME.$$"
    fi
    
    exit $exit_code
}

# Signal handlers
trap cleanup EXIT
trap 'log_error "Interrupted"; exit 130' INT
trap 'log_error "Terminated"; exit 143' TERM

# String manipulation examples
string_examples() {
    local str="Hello, World!"
    
    # Length
    echo "Length: ${#str}"
    
    # Substring
    echo "Substring: ${str:0:5}"
    
    # Replace
    echo "Replace: ${str/World/Bash}"
    
    # To uppercase
    echo "Uppercase: ${str^^}"
    
    # To lowercase
    echo "Lowercase: ${str,,}"
    
    # Remove prefix
    local path="/home/user/file.txt"
    echo "Filename: ${path##*/}"
    
    # Remove suffix
    echo "Directory: ${path%/*}"
    
    # Default value
    local empty=""
    echo "Default: ${empty:-default_value}"
}

# Array examples
array_examples() {
    # Indexed array
    local -a fruits=("apple" "banana" "cherry")
    
    echo "First fruit: ${fruits[0]}"
    echo "All fruits: ${fruits[*]}"
    echo "Number of fruits: ${#fruits[@]}"
    
    # Append to array
    fruits+=("date")
    
    # Iterate
    for fruit in "${fruits[@]}"; do
        echo "  - $fruit"
    done
    
    # Associative array
    local -A scores=(
        [alice]=100
        [bob]=85
        [carol]=92
    )
    
    echo "Alice's score: ${scores[alice]}"
    echo "All names: ${!scores[*]}"
    
    # Iterate associative array
    for name in "${!scores[@]}"; do
        echo "  $name: ${scores[$name]}"
    done
}

# Arithmetic examples
arithmetic_examples() {
    local a=10 b=3
    
    # Arithmetic expansion
    echo "Sum: $((a + b))"
    echo "Difference: $((a - b))"
    echo "Product: $((a * b))"
    echo "Quotient: $((a / b))"
    echo "Remainder: $((a % b))"
    echo "Power: $((a ** 2))"
    
    # Increment/decrement
    ((a++))
    ((b--))
    
    # Compound assignment
    ((a += 5))
    
    # Floating point with bc
    local result
    result=$(echo "scale=2; $a / $b" | bc)
    echo "Float division: $result"
}

# Conditional examples
conditional_examples() {
    local num=42
    local str="hello"
    local file="$0"
    
    # Numeric comparison
    if ((num > 0)); then
        echo "Positive"
    fi
    
    # String comparison
    if [[ "$str" == "hello" ]]; then
        echo "Greeting detected"
    fi
    
    # Regex matching
    if [[ "$str" =~ ^h.+o$ ]]; then
        echo "Pattern matched"
    fi
    
    # File tests
    if [[ -f "$file" ]]; then
        echo "File exists"
    fi
    
    if [[ -r "$file" && -x "$file" ]]; then
        echo "File is readable and executable"
    fi
    
    # Case statement
    case "$str" in
        hello|hi)
            echo "Informal greeting"
            ;;
        "good morning"|"good evening")
            echo "Formal greeting"
            ;;
        *)
            echo "Unknown"
            ;;
    esac
}

# Main function
main() {
    parse_args "$@"
    
    if [[ ${#ARGS[@]} -eq 0 ]]; then
        usage
        exit 1
    fi
    
    local command="${ARGS[0]}"
    
    case "$command" in
        process)
            if [[ ${#ARGS[@]} -lt 2 ]]; then
                log_error "process requires a file argument"
                exit 1
            fi
            process_file "${ARGS[1]}"
            ;;
        analyze)
            if [[ ${#ARGS[@]} -lt 2 ]]; then
                log_error "analyze requires a directory argument"
                exit 1
            fi
            analyze_dir "${ARGS[1]}"
            ;;
        cleanup)
            log_info "Running cleanup..."
            ;;
        *)
            log_error "Unknown command: $command"
            usage
            exit 1
            ;;
    esac
}

# Run main if not sourced
if [[ "${BASH_SOURCE[0]}" == "${0}" ]]; then
    main "$@"
fi
