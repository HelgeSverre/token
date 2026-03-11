;; Fennel Syntax Highlighting Test
;; A Neovim plugin for task management with Lua interop.

;; ============================================================
;; Module setup
;; ============================================================

(local M {})

(local version "1.0.0")
(local default-config
  {:keymaps {:toggle "<leader>tt"
             :add    "<leader>ta"
             :done   "<leader>td"
             :delete "<leader>tx"
             :filter "<leader>tf"}
   :ui {:width 60
        :height 20
        :border "rounded"
        :position "center"
        :title "Tasks"}
   :priorities {:low    {:icon " " :hl "DiagnosticInfo"}
                :medium {:icon "!" :hl "DiagnosticWarn"}
                :high   {:icon "!!" :hl "DiagnosticError"}
                :critical {:icon "!!!" :hl "DiagnosticError"}}
   :storage {:path (.. (vim.fn.stdpath "data") "/tasks.json")
             :auto-save true}})

;; ============================================================
;; State
;; ============================================================

(var state {:tasks []
            :next-id 1
            :filter-status nil
            :filter-tag nil
            :buf nil
            :win nil
            :config {}})

;; ============================================================
;; Utility functions
;; ============================================================

(fn tbl-clone [t]
  "Deep clone a table."
  (if (= (type t) :table)
    (collect [k v (pairs t)]
      (values k (tbl-clone v)))
    t))

(fn tbl-merge [base overlay]
  "Deep merge overlay into base."
  (let [result (tbl-clone base)]
    (each [k v (pairs overlay)]
      (if (and (= (type v) :table)
               (= (type (. result k)) :table))
        (tset result k (tbl-merge (. result k) v))
        (tset result k v)))
    result))

(fn find-index [tbl pred]
  "Find the index of first element matching predicate."
  (var idx nil)
  (each [i v (ipairs tbl) &until idx]
    (when (pred v)
      (set idx i)))
  idx)

(fn filter [tbl pred]
  "Return new table with elements matching predicate."
  (icollect [_ v (ipairs tbl)]
    (when (pred v) v)))

(fn map [tbl f]
  "Map function over table elements."
  (icollect [_ v (ipairs tbl)]
    (f v)))

(fn count [tbl pred]
  "Count elements matching predicate."
  (accumulate [n 0 _ v (ipairs tbl)]
    (if (pred v) (+ n 1) n)))

(fn group-by [tbl key-fn]
  "Group elements by key function."
  (let [result {}]
    (each [_ v (ipairs tbl)]
      (let [k (key-fn v)]
        (when (not (. result k))
          (tset result k []))
        (table.insert (. result k) v)))
    result))

;; ============================================================
;; Task operations
;; ============================================================

(fn priority-value [p]
  (match p
    :low 0
    :medium 1
    :high 2
    :critical 3
    _ 0))

(fn create-task [title ?opts]
  "Create a new task and add to state."
  (let [opts (or ?opts {})
        task {:id state.next-id
              :title title
              :description (or opts.description "")
              :status :open
              :priority (or opts.priority :medium)
              :tags (or opts.tags [])
              :created-at (os.time)}]
    (table.insert state.tasks task)
    (set state.next-id (+ state.next-id 1))
    task))

(fn find-task [id]
  "Find a task by ID."
  (let [idx (find-index state.tasks #(= $.id id))]
    (when idx
      (values (. state.tasks idx) idx))))

(fn update-task [id updates]
  "Update task fields."
  (let [(task idx) (find-task id)]
    (when task
      (each [k v (pairs updates)]
        (tset task k v))
      task)))

(fn complete-task [id]
  "Mark a task as done."
  (let [(task _) (find-task id)]
    (when task
      ;; Validate state transition
      (match task.status
        :open (update-task id {:status :done})
        :in_progress (update-task id {:status :done})
        _ (vim.notify
            (string.format "Cannot complete task in '%s' status" task.status)
            vim.log.levels.WARN)))))

(fn delete-task [id]
  "Remove a task."
  (let [idx (find-index state.tasks #(= $.id id))]
    (when idx
      (table.remove state.tasks idx)
      true)))

(fn filtered-tasks []
  "Get tasks matching current filters."
  (var tasks state.tasks)
  (when state.filter-status
    (set tasks (filter tasks #(= $.status state.filter-status))))
  (when state.filter-tag
    (set tasks (filter tasks
      #(let [tags $.tags]
         (accumulate [found false _ t (ipairs tags) &until found]
           (= t state.filter-tag))))))
  ;; Sort by priority descending
  (table.sort tasks
    #(> (priority-value $1.priority) (priority-value $2.priority)))
  tasks)

;; ============================================================
;; Statistics
;; ============================================================

(fn compute-stats []
  "Compute task statistics."
  (let [tasks state.tasks
        total (length tasks)
        by-status (group-by tasks #$.status)
        by-priority (group-by tasks #$.priority)
        done-count (count tasks #(= $.status :done))]
    {:total total
     :by-status (collect [k v (pairs by-status)]
                  (values k (length v)))
     :by-priority (collect [k v (pairs by-priority)]
                    (values k (length v)))
     :completion-rate (if (> total 0)
                        (* (/ done-count total) 100)
                        0)}))

;; ============================================================
;; UI rendering
;; ============================================================

(fn format-task [task]
  "Format a task for display."
  (let [cfg state.config
        icon (match task.status
               :open "[ ]"
               :in_progress "[~]"
               :done "[x]"
               :cancelled "[-]")
        prio-cfg (. cfg.priorities task.priority)
        prio-icon (or prio-cfg.icon " ")
        tag-str (if (> (length task.tags) 0)
                  (.. " [" (table.concat task.tags ", ") "]")
                  "")]
    (string.format "#%d %s %s %s%s"
      task.id icon prio-icon task.title tag-str)))

(fn render-task-list []
  "Render the task list in the floating window."
  (when (and state.buf (vim.api.nvim_buf_is_valid state.buf))
    (let [tasks (filtered-tasks)
          lines (if (> (length tasks) 0)
                  (map tasks format-task)
                  ["  No tasks found."])
          ;; Add header
          stats (compute-stats)
          header [(string.format " Tasks (%d total, %.0f%% done)"
                    stats.total stats.completion-rate)
                  (string.rep "─" state.config.ui.width)]]
      ;; Set buffer content
      (vim.api.nvim_buf_set_option state.buf :modifiable true)
      (vim.api.nvim_buf_set_lines state.buf 0 -1 false
        (icollect [_ l (ipairs header)]
          l))
      (vim.api.nvim_buf_set_lines state.buf -1 -1 false
        (icollect [_ l (ipairs lines)]
          (.. "  " l)))
      (vim.api.nvim_buf_set_option state.buf :modifiable false)

      ;; Apply highlights
      (each [i task (ipairs tasks)]
        (let [line (+ i (length header))
              prio-cfg (. state.config.priorities task.priority)]
          (when prio-cfg
            (vim.api.nvim_buf_add_highlight
              state.buf -1 prio-cfg.hl (- line 1) 0 -1)))))))

(fn open-window []
  "Open the floating task window."
  (let [cfg state.config.ui
        buf (vim.api.nvim_create_buf false true)
        win-opts {:relative "editor"
                  :width cfg.width
                  :height cfg.height
                  :col (math.floor (/ (- vim.o.columns cfg.width) 2))
                  :row (math.floor (/ (- vim.o.lines cfg.height) 2))
                  :style "minimal"
                  :border cfg.border
                  :title (.. " " cfg.title " ")
                  :title_pos "center"}
        win (vim.api.nvim_open_win buf true win-opts)]
    (set state.buf buf)
    (set state.win win)

    ;; Buffer options
    (vim.api.nvim_buf_set_option buf :bufhidden "wipe")
    (vim.api.nvim_buf_set_option buf :filetype "tasks")

    ;; Keymaps for the task buffer
    (let [opts {:buffer buf :silent true}]
      (vim.keymap.set :n "q" #(close-window) opts)
      (vim.keymap.set :n "<Esc>" #(close-window) opts)
      (vim.keymap.set :n "a" #(prompt-add-task) opts)
      (vim.keymap.set :n "d" #(prompt-complete-task) opts)
      (vim.keymap.set :n "x" #(prompt-delete-task) opts)
      (vim.keymap.set :n "f" #(cycle-filter) opts))

    (render-task-list)))

(fn close-window []
  "Close the floating window."
  (when (and state.win (vim.api.nvim_win_is_valid state.win))
    (vim.api.nvim_win_close state.win true))
  (set state.win nil)
  (set state.buf nil))

(fn toggle-window []
  "Toggle the task window."
  (if (and state.win (vim.api.nvim_win_is_valid state.win))
    (close-window)
    (open-window)))

;; ============================================================
;; Interactive prompts
;; ============================================================

(fn prompt-add-task []
  (vim.ui.input {:prompt "Task title: "}
    (fn [title]
      (when (and title (> (length title) 0))
        (vim.ui.select [:low :medium :high :critical]
          {:prompt "Priority:"}
          (fn [priority]
            (when priority
              (create-task title {:priority priority})
              (render-task-list)
              (vim.notify (.. "Created: " title)
                vim.log.levels.INFO))))))))

(fn prompt-complete-task []
  (vim.ui.input {:prompt "Task ID to complete: "}
    (fn [input]
      (when input
        (let [id (tonumber input)]
          (if id
            (do (complete-task id)
                (render-task-list))
            (vim.notify "Invalid ID" vim.log.levels.ERROR)))))))

(fn prompt-delete-task []
  (vim.ui.input {:prompt "Task ID to delete: "}
    (fn [input]
      (when input
        (let [id (tonumber input)]
          (if (and id (delete-task id))
            (do (render-task-list)
                (vim.notify (.. "Deleted task #" input)))
            (vim.notify "Task not found" vim.log.levels.ERROR)))))))

(fn cycle-filter []
  (let [statuses [nil :open :in_progress :done :cancelled]
        current state.filter-status
        idx (or (find-index statuses #(= $ current)) 0)
        next-idx (% (+ idx 1) (+ (length statuses) 1))]
    (set state.filter-status (. statuses (+ next-idx 1)))
    (render-task-list)
    (vim.notify
      (if state.filter-status
        (.. "Filter: " state.filter-status)
        "Filter: all")
      vim.log.levels.INFO)))

;; ============================================================
;; Plugin setup
;; ============================================================

(fn M.setup [?user-config]
  "Initialize the task manager plugin."
  (set state.config (tbl-merge default-config (or ?user-config {})))

  ;; Register keymaps
  (let [km state.config.keymaps]
    (vim.keymap.set :n km.toggle toggle-window
      {:desc "Toggle task list"})
    (vim.keymap.set :n km.add prompt-add-task
      {:desc "Add new task"}))

  ;; Register user commands
  (vim.api.nvim_create_user_command "TaskAdd"
    (fn [opts] (create-task opts.args) (render-task-list))
    {:nargs 1 :desc "Add a task"})

  (vim.api.nvim_create_user_command "TaskList"
    (fn [_] (toggle-window))
    {:desc "Toggle task list"})

  (vim.api.nvim_create_user_command "TaskStats"
    (fn [_]
      (let [stats (compute-stats)]
        (vim.notify
          (string.format "Tasks: %d | Done: %.0f%% | Open: %d"
            stats.total
            stats.completion-rate
            (or (. stats.by-status :open) 0))
          vim.log.levels.INFO)))
    {:desc "Show task statistics"})

  (vim.notify (string.format "Task Manager v%s loaded" version)
    vim.log.levels.INFO))

M
