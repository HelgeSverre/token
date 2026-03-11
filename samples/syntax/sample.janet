# Janet Syntax Highlighting Test
# A PEG parser and task manager with fibers, macros, and pattern matching.

(def version "1.0.0")
(def max-tasks 1000)

# ============================================================
# PEG grammar for a simple config format
# ============================================================

(def config-grammar
  ~{:ws (set " \t\r\n")
    :comment (* "#" (any (if-not "\n" 1)) (+ "\n" -1))
    :_ (any (+ :ws :comment))

    # Primitives
    :null (* "null" (constant nil))
    :true (* "true" (constant true))
    :false (* "false" (constant false))
    :boolean (+ :true :false)

    :digit (range "09")
    :int (/ (* (? "-") (some :digit)) ,scan-number)
    :float (/ (* (? "-") (some :digit) "." (some :digit)) ,scan-number)
    :number (+ :float :int)

    :escape (* "\\" (set "\"\\nrt/"))
    :char (+ :escape (if-not "\"" 1))
    :string (* "\"" (/ (capture (any :char)) ,identity) "\"")

    # Identifiers and keys
    :ident-char (+ (range "az" "AZ" "09") (set "-_"))
    :identifier (capture (some :ident-char))

    # Collections
    :array-items (* :value (any (* :_ "," :_ :value)))
    :array (/ (* "[" :_ (? :array-items) :_ "]") ,tuple)

    :pair (* :_ (+ :string :identifier) :_ ":" :_ :value)
    :object-items (* :pair (any (* :_ "," :_ :pair)))
    :object (/ (* "{" :_ (? :object-items) :_ "}" ) ,struct)

    # Value
    :value (* :_ (+ :null :boolean :number :string :array :object) :_)

    # Top-level: key = value pairs
    :assignment (/ (* :identifier :_ "=" :_ :value) ,|[$0 $1])
    :main (* (any (* :_ :assignment :_)) :_ -1)})

(defn parse-config
  "Parse a configuration string into a table."
  [input]
  (def result (peg/match config-grammar input))
  (if result
    (do
      (def tbl @{})
      (each [k v] result
        (put tbl (keyword k) v))
      tbl)
    (error "Failed to parse configuration")))

# ============================================================
# Task data structures
# ============================================================

(defn make-task
  "Create a new task table."
  [id title &named priority tags description]
  (default priority :medium)
  (default tags @[])
  (default description "")
  @{:id id
    :title title
    :description description
    :status :open
    :priority priority
    :tags (array/slice tags)
    :created-at (os/time)})

(defn priority-value [p]
  (case p
    :low 0
    :medium 1
    :high 2
    :critical 3
    0))

(defn status-icon [s]
  (case s
    :open "[ ]"
    :in-progress "[~]"
    :done "[x]"
    :cancelled "[-]"
    "[?]"))

(defn priority-icon [p]
  (case p
    :low " "
    :medium "!"
    :high "!!"
    :critical "!!!"
    "?"))

(defn format-task [task]
  (def tags (task :tags))
  (def tag-str
    (if (> (length tags) 0)
      (string " [" (string/join tags ", ") "]")
      ""))
  (string/format "#%d %s %s %s%s"
    (task :id)
    (status-icon (task :status))
    (priority-icon (task :priority))
    (task :title)
    tag-str))

# ============================================================
# Task store
# ============================================================

(defn make-store []
  @{:tasks @{}
    :next-id 1})

(defn store/create [store title &named priority tags]
  (default priority :medium)
  (default tags @[])
  (def id (store :next-id))
  (def task (make-task id title :priority priority :tags tags))
  (put-in store [:tasks id] task)
  (put store :next-id (inc id))
  task)

(defn store/get [store id]
  (get-in store [:tasks id]))

(defn store/update-status [store id new-status]
  (def task (store/get store id))
  (unless task (error (string/format "Task %d not found" id)))

  # Validate transition
  (def valid-transitions
    {:open [:in-progress :cancelled]
     :in-progress [:open :done :cancelled]
     :done [:open]
     :cancelled [:open]})

  (def allowed (get valid-transitions (task :status)))
  (unless (find |(= $ new-status) allowed)
    (errorf "Cannot transition from %s to %s"
      (task :status) new-status))

  (put task :status new-status)
  task)

(defn store/delete [store id]
  (def tasks (store :tasks))
  (if (in tasks id)
    (do (put tasks id nil) true)
    false))

(defn store/all [store]
  (def tasks (values (store :tasks)))
  (sort-by |(- (priority-value ($ :priority))) tasks))

(defn store/filter-by-status [store status]
  (filter |(= ($ :status) status) (store/all store)))

(defn store/filter-by-tag [store tag]
  (filter |(find |(= $ tag) ($ :tags)) (store/all store)))

# ============================================================
# Statistics
# ============================================================

(defn compute-stats [store]
  (def tasks (store/all store))
  (def total (length tasks))

  (def by-status @{})
  (def by-priority @{})
  (var done-count 0)
  (var total-tags 0)

  (each task tasks
    (update by-status (task :status) |(inc (or $ 0)))
    (update by-priority (task :priority) |(inc (or $ 0)))
    (when (= (task :status) :done) (++ done-count))
    (+= total-tags (length (task :tags))))

  {:total total
   :by-status (freeze by-status)
   :by-priority (freeze by-priority)
   :completion-rate (if (> total 0)
                      (* (/ done-count total) 100)
                      0)
   :avg-tags (if (> total 0)
               (/ total-tags total)
               0)})

(defn print-stats [stats]
  (printf "\n=== Statistics ===")
  (printf "Total: %d" (stats :total))
  (printf "Completion: %.1f%%" (stats :completion-rate))
  (printf "\nBy status:")
  (eachp [status count] (stats :by-status)
    (printf "  %s: %d" status count))
  (printf "\nBy priority:")
  (eachp [priority count] (stats :by-priority)
    (printf "  %s: %d" priority count)))

# ============================================================
# Macros
# ============================================================

(defmacro with-timing
  "Execute body and print elapsed time."
  [label & body]
  ~(do
     (def _start (os/clock))
     (def _result (do ,;body))
     (def _elapsed (* (- (os/clock) _start) 1000))
     (printf "%s: %.2fms" ,label _elapsed)
     _result))

(defmacro defcommand
  "Define a named command with documentation."
  [name docstring args & body]
  ~(defn ,name ,docstring ,args
     (try
       (do ,;body)
       ([err fib]
        (eprintf "Command '%s' failed: %s\n" ,(string name) err)))))

# ============================================================
# Fiber-based batch processing
# ============================================================

(defn make-worker
  "Create a fiber that processes tasks from a channel."
  [id process-fn]
  (fiber/new
    (fn []
      (var running true)
      (while running
        (def task (yield))
        (if (nil? task)
          (set running false)
          (do
            (printf "  Worker %d: processing '%s'" id (task :title))
            (process-fn task)))))))

(defn batch-process
  "Process tasks using a pool of worker fibers."
  [tasks n-workers process-fn]
  (def workers
    (seq [i :range [0 n-workers]]
      (make-worker i process-fn)))

  (var worker-idx 0)
  (each task tasks
    (def worker (workers worker-idx))
    (resume worker task)
    (set worker-idx (% (inc worker-idx) n-workers)))

  # Signal completion
  (each worker workers
    (resume worker nil)))

# ============================================================
# Main
# ============================================================

(defcommand run-demo
  "Run the task manager demo."
  []

  (printf "Task Manager v%s\n" version)

  (def store (make-store))

  # Create tasks
  (store/create store "Implement syntax highlighting"
    :priority :high :tags @["feature" "syntax"])
  (store/create store "Fix cursor blinking"
    :priority :low :tags @["bug"])
  (store/create store "Add split view"
    :priority :medium :tags @["feature" "ui"])
  (store/create store "Write documentation"
    :priority :medium :tags @["docs"])
  (store/create store "Performance profiling"
    :priority :high :tags @["perf"])

  # Transition tasks
  (store/update-status store 1 :in-progress)
  (store/update-status store 2 :done)

  # Display
  (print "\nAll tasks:")
  (each task (store/all store)
    (printf "  %s" (format-task task)))

  # Stats
  (def stats (compute-stats store))
  (print-stats stats)

  # Config parsing demo
  (print "\n=== Config Parse Demo ===")
  (def config-str `
    host = "localhost"
    port = 8080
    debug = true
    tags = ["web", "api"]
  `)

  (with-timing "Config parse"
    (def config (parse-config config-str))
    (printf "Parsed: %q" config)))

# Run
(run-demo)
