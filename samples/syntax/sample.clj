;; Clojure Syntax Highlighting Test
;; A data transformation pipeline with transducers, multimethods, and macros.

(ns sample.pipeline
  (:require [clojure.string :as str]
            [clojure.set :as set]
            [clojure.edn :as edn]
            [clojure.java.io :as io]
            [clojure.core.async :as async :refer [go go-loop <! >! chan close!]])
  (:import [java.time Instant Duration LocalDate]
           [java.util UUID]))

;; ============================================================
;; Constants and configuration
;; ============================================================

(def ^:const version "1.0.0")
(def ^:const max-batch-size 1000)
(def ^:dynamic *log-level* :info)

(def config
  {:database {:host "localhost"
              :port 5432
              :name "analytics"
              :pool-size 10}
   :processing {:batch-size 100
                :parallelism (.availableProcessors (Runtime/getRuntime))
                :timeout-ms 30000}
   :features #{:dedup :enrichment :validation}})

;; ============================================================
;; Protocols and records
;; ============================================================

(defprotocol Transformable
  "Protocol for data that can be transformed through the pipeline."
  (transform [this ctx] "Apply transformation with context")
  (validate [this] "Returns {:ok data} or {:error reason}")
  (serialize [this format] "Serialize to the given format"))

(defrecord Event [id type timestamp payload source]
  Transformable
  (transform [this ctx]
    (-> this
        (update :payload merge (:enrichment ctx))
        (assoc :processed-at (Instant/now))))

  (validate [this]
    (cond
      (str/blank? (:type this))
      {:error "Event type is required"}

      (nil? (:timestamp this))
      {:error "Timestamp is required"}

      (> (count (pr-str (:payload this))) 65536)
      {:error "Payload too large"}

      :else
      {:ok this}))

  (serialize [this format]
    (case format
      :edn (pr-str this)
      :json (str "{\"id\":\"" (:id this) "\","
                 "\"type\":\"" (:type this) "\","
                 "\"timestamp\":\"" (:timestamp this) "\"}")
      (throw (ex-info "Unknown format" {:format format})))))

;; ============================================================
;; Multimethods - dispatch on event type
;; ============================================================

(defmulti process-event
  "Process an event based on its type."
  (fn [event _ctx] (:type event)))

(defmethod process-event :user/login
  [event ctx]
  (let [{:keys [user-id ip-address]} (:payload event)]
    (println (format "  Login: user=%s ip=%s" user-id ip-address))
    (assoc-in event [:payload :session-id] (str (UUID/randomUUID)))))

(defmethod process-event :user/action
  [event ctx]
  (let [{:keys [action resource]} (:payload event)]
    (println (format "  Action: %s on %s" action resource))
    (update-in event [:payload :action-count] (fnil inc 0))))

(defmethod process-event :system/metric
  [event _ctx]
  (let [{:keys [name value unit]} (:payload event)
        threshold (get-in ctx [:thresholds name] ##Inf)]
    (when (> value threshold)
      (println (format "  ALERT: %s = %.2f %s (threshold: %.2f)"
                       name (double value) (or unit "") (double threshold))))
    event))

(defmethod process-event :default
  [event _ctx]
  (println (format "  Unknown event type: %s" (:type event)))
  event)

;; ============================================================
;; Transducers for efficient pipeline composition
;; ============================================================

(defn valid-event?
  "Predicate: is this event valid?"
  [event]
  (let [result (validate event)]
    (when-let [err (:error result)]
      (println (format "  Skipping invalid event %s: %s" (:id event) err)))
    (:ok result)))

(defn dedup-xf
  "Stateful transducer that deduplicates events by ID."
  []
  (fn [rf]
    (let [seen (volatile! #{})]
      (fn
        ([] (rf))
        ([result] (rf result))
        ([result event]
         (let [id (:id event)]
           (if (@seen id)
             (do (println (format "  Dedup: skipping %s" id))
                 result)
             (do (vswap! seen conj id)
                 (rf result event)))))))))

(defn batch-xf
  "Transducer that batches items into vectors of size n."
  [n]
  (fn [rf]
    (let [batch (volatile! [])]
      (fn
        ([] (rf))
        ([result]
         (let [b @batch]
           (if (seq b)
             (rf (rf result b))
             (rf result))))
        ([result item]
         (let [b (vswap! batch conj item)]
           (if (= (count b) n)
             (do (vreset! batch [])
                 (rf result b))
             result)))))))

(def pipeline-xf
  "Composed transducer for the full pipeline."
  (comp
    (filter valid-event?)
    (dedup-xf)
    (map #(transform % {:enrichment {:pipeline-version version}}))
    (batch-xf (:batch-size (:processing config)))))

;; ============================================================
;; Async processing with core.async
;; ============================================================

(defn start-processor
  "Start an async event processor. Returns a map of channels."
  [{:keys [parallelism] :as opts}]
  (let [input-ch  (chan 1024)
        output-ch (chan 1024)
        error-ch  (chan 256)
        done-ch   (chan)]

    ;; Worker pool
    (dotimes [worker-id parallelism]
      (go-loop []
        (when-let [batch (<! input-ch)]
          (try
            (let [results (mapv #(process-event % opts) batch)
                  successful (filter some? results)]
              (doseq [result successful]
                (>! output-ch result)))
            (catch Exception e
              (>! error-ch {:worker worker-id
                            :error (.getMessage e)
                            :batch-size (count batch)})))
          (recur))))

    ;; Collector
    (go-loop [total 0]
      (if-let [event (<! output-ch)]
        (do
          (when (zero? (mod (inc total) 100))
            (println (format "Processed %d events" (inc total))))
          (recur (inc total)))
        (do
          (>! done-ch {:total total})
          (close! done-ch))))

    {:input input-ch
     :output output-ch
     :error error-ch
     :done done-ch}))

;; ============================================================
;; Macros
;; ============================================================

(defmacro with-timing
  "Execute body and print elapsed time."
  [label & body]
  `(let [start# (System/nanoTime)
         result# (do ~@body)
         elapsed# (/ (- (System/nanoTime) start#) 1e6)]
     (println (format "%s: %.2fms" ~label elapsed#))
     result#))

(defmacro defpipe
  "Define a named pipeline step with automatic logging."
  [name doc bindings & body]
  `(defn ~name ~doc ~bindings
     (with-timing ~(str name)
       ~@body)))

;; ============================================================
;; Pipeline steps using the macro
;; ============================================================

(defpipe enrich-events
  "Enrich events with external data."
  [events lookup-fn]
  (->> events
       (map (fn [event]
              (if-let [extra (lookup-fn (:source event))]
                (update event :payload merge extra)
                event)))
       (into [])))

(defpipe aggregate-metrics
  "Aggregate metric events by name."
  [events]
  (->> events
       (filter #(= :system/metric (:type %)))
       (group-by #(get-in % [:payload :name]))
       (reduce-kv
         (fn [acc metric-name events]
           (let [values (map #(get-in % [:payload :value]) events)]
             (assoc acc metric-name
                    {:count (count values)
                     :min (apply min values)
                     :max (apply max values)
                     :mean (/ (reduce + values) (count values))
                     :sum (reduce + values)})))
         {})))

;; ============================================================
;; Main
;; ============================================================

(defn generate-events
  "Generate n random events for testing."
  [n]
  (let [types [:user/login :user/action :system/metric]
        sources ["web" "mobile" "api" "batch"]]
    (repeatedly n
      #(->Event
         (str (UUID/randomUUID))
         (rand-nth types)
         (Instant/now)
         (case (rand-nth types)
           :user/login   {:user-id (str "user-" (rand-int 100))
                          :ip-address (str/join "." (repeatedly 4 #(rand-int 256)))}
           :user/action  {:action (rand-nth ["click" "scroll" "submit" "navigate"])
                          :resource (str "/page/" (rand-int 50))}
           :system/metric {:name (rand-nth ["cpu" "memory" "latency" "throughput"])
                           :value (* (rand) 100.0)
                           :unit (rand-nth ["%" "MB" "ms" "req/s"])})
         (rand-nth sources)))))

(defn -main
  [& args]
  (println (format "Event Pipeline v%s" version))
  (println (format "Config: %s workers, batch size %d"
                   (:parallelism (:processing config))
                   (:batch-size (:processing config))))

  (with-timing "Total pipeline"
    (let [events (generate-events 500)
          processed (into [] pipeline-xf events)
          metrics (aggregate-metrics (apply concat processed))]

      (println "\n=== Metrics Summary ===")
      (doseq [[name stats] (sort-by key metrics)]
        (println (format "  %s: count=%d min=%.2f max=%.2f mean=%.2f"
                         name (:count stats) (:min stats) (:max stats) (:mean stats))))

      (println (format "\nTotal batches: %d" (count processed)))
      (println (format "Total events: %d" (reduce + (map count processed)))))))
