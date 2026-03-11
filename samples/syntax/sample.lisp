;;;; Common Lisp Syntax Highlighting Test
;;;; A task manager with CLOS, conditions, macros, and reader macros.

(defpackage #:task-manager
  (:use #:cl)
  (:export #:make-task #:task-store #:create-task #:complete-task
           #:all-tasks #:compute-stats #:run-demo))

(in-package #:task-manager)

;;; ============================================================
;;; Constants and special variables
;;; ============================================================

(defconstant +version+ "1.0.0")
(defconstant +max-tasks+ 10000)

(defparameter *default-priority* :medium)
(defparameter *verbose* nil)

;;; ============================================================
;;; Conditions (Common Lisp's condition system)
;;; ============================================================

(define-condition task-error (error)
  ((task-id :initarg :task-id :reader task-error-id)
   (message :initarg :message :reader task-error-message))
  (:report (lambda (condition stream)
             (format stream "Task error (#~A): ~A"
                     (task-error-id condition)
                     (task-error-message condition)))))

(define-condition task-not-found (task-error) ()
  (:report (lambda (condition stream)
             (format stream "Task #~A not found" (task-error-id condition)))))

(define-condition invalid-transition (task-error)
  ((from-status :initarg :from :reader transition-from)
   (to-status :initarg :to :reader transition-to))
  (:report (lambda (condition stream)
             (format stream "Cannot transition task #~A from ~A to ~A"
                     (task-error-id condition)
                     (transition-from condition)
                     (transition-to condition)))))

;;; ============================================================
;;; CLOS: Task class hierarchy
;;; ============================================================

(deftype priority () '(member :low :medium :high :critical))
(deftype status () '(member :open :in-progress :done :cancelled))

(defclass task ()
  ((id          :initarg :id          :accessor task-id          :type integer)
   (title       :initarg :title       :accessor task-title       :type string)
   (description :initarg :description :accessor task-description :type string
                :initform "")
   (status      :initarg :status      :accessor task-status      :type status
                :initform :open)
   (priority    :initarg :priority    :accessor task-priority     :type priority
                :initform :medium)
   (tags        :initarg :tags        :accessor task-tags         :type list
                :initform nil)
   (created-at  :initarg :created-at  :accessor task-created-at
                :initform (get-universal-time)))
  (:documentation "A task with status, priority, and tags."))

;;; Generic functions with multiple dispatch
(defgeneric format-item (item &key stream verbose)
  (:documentation "Format an item for display."))

(defgeneric priority-value (priority)
  (:documentation "Numeric value of a priority for sorting."))

;;; Methods
(defmethod priority-value ((p (eql :low)))      0)
(defmethod priority-value ((p (eql :medium)))    1)
(defmethod priority-value ((p (eql :high)))      2)
(defmethod priority-value ((p (eql :critical)))  3)

(defmethod print-object ((task task) stream)
  (print-unreadable-object (task stream :type t :identity nil)
    (format stream "#~D ~A [~A]" (task-id task) (task-title task) (task-status task))))

(defmethod format-item ((task task) &key (stream *standard-output*) verbose)
  (let ((icon (ecase (task-status task)
                (:open "[ ]")
                (:in-progress "[~]")
                (:done "[x]")
                (:cancelled "[-]")))
        (prio (ecase (task-priority task)
                (:low " ")
                (:medium "!")
                (:high "!!")
                (:critical "!!!")))
        (tag-str (if (task-tags task)
                     (format nil " [~{~A~^, ~}]" (task-tags task))
                     "")))
    (format stream "  #~D ~A ~A ~A~A~%"
            (task-id task) icon prio (task-title task) tag-str)
    (when (and verbose (string/= (task-description task) ""))
      (format stream "       ~A~%" (task-description task)))))

;;; State machine for status transitions
(defparameter *valid-transitions*
  '((:open        :in-progress :cancelled)
    (:in-progress :open :done :cancelled)
    (:done        :open)
    (:cancelled   :open)))

(defun valid-transition-p (from to)
  "Check if transitioning from FROM to TO is valid."
  (member to (cdr (assoc from *valid-transitions*))))

(defun transition-task (task new-status)
  "Transition a task to a new status, signaling on invalid transition."
  (let ((current (task-status task)))
    (unless (valid-transition-p current new-status)
      (restart-case
          (error 'invalid-transition
                 :task-id (task-id task)
                 :from current
                 :to new-status)
        (force-transition ()
          :report "Force the transition anyway"
          nil)
        (keep-current ()
          :report "Keep the current status"
          (return-from transition-task task))))
    (setf (task-status task) new-status)
    task))

;;; ============================================================
;;; Task store
;;; ============================================================

(defclass task-store ()
  ((tasks   :initform (make-hash-table) :accessor store-tasks)
   (next-id :initform 1                 :accessor store-next-id))
  (:documentation "Thread-safe task storage."))

(defmethod create-task ((store task-store) title &key
                        (priority *default-priority*)
                        (tags nil))
  "Create and store a new task."
  (let* ((id (store-next-id store))
         (task (make-instance 'task
                              :id id :title title
                              :priority priority :tags tags)))
    (setf (gethash id (store-tasks store)) task)
    (incf (store-next-id store))
    task))

(defmethod get-task ((store task-store) id)
  "Get a task by ID, signaling if not found."
  (or (gethash id (store-tasks store))
      (error 'task-not-found :task-id id :message "not found")))

(defmethod delete-task ((store task-store) id)
  "Remove a task from the store."
  (remhash id (store-tasks store)))

(defmethod all-tasks ((store task-store) &key (sort-by :priority))
  "Return all tasks, optionally sorted."
  (let ((tasks (loop for task being the hash-values of (store-tasks store)
                     collect task)))
    (ecase sort-by
      (:priority (sort tasks #'> :key (lambda (t) (priority-value (task-priority t)))))
      (:id       (sort tasks #'< :key #'task-id))
      (:status   (sort tasks #'string< :key (lambda (t) (symbol-name (task-status t)))))
      ((nil)     tasks))))

(defmethod filter-tasks ((store task-store) &key status tag)
  "Filter tasks by status and/or tag."
  (loop for task being the hash-values of (store-tasks store)
        when (and (or (null status) (eq (task-status task) status))
                  (or (null tag) (member tag (task-tags task) :test #'string=)))
          collect task))

;;; ============================================================
;;; Statistics
;;; ============================================================

(defstruct stats
  (total 0 :type integer)
  (by-status nil :type list)
  (by-priority nil :type list)
  (completion-rate 0.0 :type float))

(defmethod compute-stats ((store task-store))
  "Compute statistics for all tasks."
  (let ((tasks (all-tasks store :sort-by nil))
        (by-status (make-hash-table))
        (by-priority (make-hash-table))
        (done-count 0))
    (dolist (task tasks)
      (incf (gethash (task-status task) by-status 0))
      (incf (gethash (task-priority task) by-priority 0))
      (when (eq (task-status task) :done)
        (incf done-count)))
    (make-stats
     :total (length tasks)
     :by-status (loop for k being the hash-keys of by-status
                        using (hash-value v)
                      collect (cons k v))
     :by-priority (loop for k being the hash-keys of by-priority
                          using (hash-value v)
                        collect (cons k v))
     :completion-rate (if (zerop (length tasks))
                          0.0
                          (* (/ done-count (length tasks)) 100.0)))))

(defmethod format-item ((stats stats) &key (stream *standard-output*) verbose)
  (declare (ignore verbose))
  (format stream "~&=== Statistics ===~%")
  (format stream "Total: ~D~%" (stats-total stats))
  (format stream "Completion: ~,1F%~%" (stats-completion-rate stats))
  (format stream "~%By status:~%")
  (dolist (pair (stats-by-status stats))
    (format stream "  ~A: ~D~%" (car pair) (cdr pair)))
  (format stream "~%By priority:~%")
  (dolist (pair (stats-by-priority stats))
    (format stream "  ~A: ~D~%" (car pair) (cdr pair))))

;;; ============================================================
;;; Macros
;;; ============================================================

(defmacro with-timing ((label) &body body)
  "Execute BODY and print elapsed time."
  (let ((start (gensym "START"))
        (result (gensym "RESULT")))
    `(let ((,start (get-internal-real-time)))
       (let ((,result (progn ,@body)))
         (format t "~A: ~,2Fms~%"
                 ,label
                 (* (/ (- (get-internal-real-time) ,start)
                       internal-time-units-per-second)
                    1000.0))
         ,result))))

(defmacro do-tasks ((var store &key status) &body body)
  "Iterate over tasks in store, optionally filtered by status."
  `(dolist (,var ,(if status
                      `(filter-tasks ,store :status ,status)
                      `(all-tasks ,store)))
     ,@body))

;;; ============================================================
;;; Main
;;; ============================================================

(defun run-demo ()
  "Run the task manager demo."
  (format t "Task Manager v~A~%~%" +version+)

  (let ((store (make-instance 'task-store)))
    ;; Create tasks
    (create-task store "Implement syntax highlighting"
                 :priority :high :tags '("feature" "syntax"))
    (create-task store "Fix cursor blinking"
                 :priority :low :tags '("bug"))
    (create-task store "Add split view"
                 :priority :medium :tags '("feature" "ui"))
    (create-task store "Write documentation"
                 :priority :medium :tags '("docs"))
    (create-task store "Performance profiling"
                 :priority :high :tags '("perf"))

    ;; Transition tasks
    (transition-task (get-task store 1) :in-progress)
    (transition-task (get-task store 2) :done)

    ;; Display
    (format t "All tasks:~%")
    (do-tasks (task store)
      (format-item task))

    ;; Stats
    (with-timing ("Stats computation")
      (let ((stats (compute-stats store)))
        (terpri)
        (format-item stats)))

    ;; Error handling demo
    (format t "~%Error handling:~%")
    (handler-case
        (transition-task (get-task store 2) :in-progress)
      (invalid-transition (c)
        (format t "  Caught: ~A~%" c)))

    (handler-case
        (get-task store 999)
      (task-not-found (c)
        (format t "  Caught: ~A~%" c)))))
