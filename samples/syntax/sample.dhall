-- Dhall Syntax Highlighting Test
-- A typed, total configuration language for a deployment pipeline.

-- ============================================================
-- Type definitions
-- ============================================================

let Priority = < Low | Medium | High | Critical >

let Status = < Open | InProgress | Done | Cancelled >

let Environment = < Development | Staging | Production >

let ResourceLimits = { cpu : Text, memory : Text }

let Resources = { requests : ResourceLimits, limits : ResourceLimits }

let HealthCheck =
      { path : Text
      , port : Natural
      , initialDelaySeconds : Natural
      , periodSeconds : Natural
      , failureThreshold : Natural
      }

let EnvVar = { name : Text, value : Text }

let ContainerConfig =
      { name : Text
      , image : Text
      , tag : Text
      , ports : List Natural
      , env : List EnvVar
      , resources : Resources
      , healthCheck : Optional HealthCheck
      }

let ServiceConfig =
      { name : Text
      , type : Text
      , port : Natural
      , targetPort : Natural
      }

let AutoscalingConfig =
      { enabled : Bool
      , minReplicas : Natural
      , maxReplicas : Natural
      , targetCPUPercent : Natural
      }

let DeploymentConfig =
      { environment : Environment
      , namespace : Text
      , replicas : Natural
      , container : ContainerConfig
      , service : ServiceConfig
      , autoscaling : AutoscalingConfig
      , labels : List { mapKey : Text, mapValue : Text }
      }

-- ============================================================
-- Helper functions
-- ============================================================

let priorityToText =
      \(p : Priority) ->
        merge
          { Low = "low"
          , Medium = "medium"
          , High = "high"
          , Critical = "critical"
          }
          p

let statusToText =
      \(s : Status) ->
        merge
          { Open = "open"
          , InProgress = "in_progress"
          , Done = "done"
          , Cancelled = "cancelled"
          }
          s

let environmentToText =
      \(e : Environment) ->
        merge
          { Development = "development"
          , Staging = "staging"
          , Production = "production"
          }
          e

let mkEnvVar =
      \(name : Text) ->
      \(value : Text) ->
        { name, value }

let mkResources =
      \(reqCpu : Text) ->
      \(reqMem : Text) ->
      \(limCpu : Text) ->
      \(limMem : Text) ->
        { requests = { cpu = reqCpu, memory = reqMem }
        , limits = { cpu = limCpu, memory = limMem }
        }

let mkHealthCheck =
      \(path : Text) ->
      \(port : Natural) ->
        { path
        , port
        , initialDelaySeconds = 10
        , periodSeconds = 30
        , failureThreshold = 3
        }

let mkLabels =
      \(app : Text) ->
      \(env : Environment) ->
        [ { mapKey = "app", mapValue = app }
        , { mapKey = "environment", mapValue = environmentToText env }
        , { mapKey = "managed-by", mapValue = "dhall" }
        ]

-- ============================================================
-- Base configuration (shared across environments)
-- ============================================================

let appName = "token-editor-api"

let baseContainer =
      { name = appName
      , image = "ghcr.io/example/token-editor-api"
      , tag = "latest"
      , ports = [ 8080, 9090 ]
      , env =
        [ mkEnvVar "APP_NAME" "token-editor"
        , mkEnvVar "LOG_FORMAT" "json"
        , mkEnvVar "METRICS_ENABLED" "true"
        ]
      , resources = mkResources "100m" "128Mi" "500m" "512Mi"
      , healthCheck = Some (mkHealthCheck "/health" 8080)
      }

let baseService =
      { name = appName
      , type = "ClusterIP"
      , port = 80
      , targetPort = 8080
      }

let baseAutoscaling =
      { enabled = False
      , minReplicas = 1
      , maxReplicas = 10
      , targetCPUPercent = 70
      }

-- ============================================================
-- Environment-specific configurations
-- ============================================================

let developmentConfig
    : DeploymentConfig
    = { environment = Environment.Development
      , namespace = "token-editor-dev"
      , replicas = 1
      , container =
            baseContainer
          //  { tag = "dev"
              , env =
                  baseContainer.env
                # [ mkEnvVar "ENVIRONMENT" "development"
                  , mkEnvVar "LOG_LEVEL" "debug"
                  , mkEnvVar "ENABLE_DEVTOOLS" "true"
                  ]
              , resources = mkResources "50m" "64Mi" "200m" "256Mi"
              }
      , service = baseService
      , autoscaling = baseAutoscaling
      , labels = mkLabels appName Environment.Development
      }

let stagingConfig
    : DeploymentConfig
    = { environment = Environment.Staging
      , namespace = "token-editor-staging"
      , replicas = 2
      , container =
            baseContainer
          //  { tag = "rc-latest"
              , env =
                  baseContainer.env
                # [ mkEnvVar "ENVIRONMENT" "staging"
                  , mkEnvVar "LOG_LEVEL" "info"
                  ]
              }
      , service = baseService
      , autoscaling = baseAutoscaling // { enabled = True, maxReplicas = 5 }
      , labels = mkLabels appName Environment.Staging
      }

let productionConfig
    : DeploymentConfig
    = { environment = Environment.Production
      , namespace = "token-editor-prod"
      , replicas = 3
      , container =
            baseContainer
          //  { tag = "v0.3.19"
              , env =
                  baseContainer.env
                # [ mkEnvVar "ENVIRONMENT" "production"
                  , mkEnvVar "LOG_LEVEL" "warn"
                  , mkEnvVar "RATE_LIMIT" "1000"
                  , mkEnvVar "CACHE_TTL" "300"
                  ]
              , resources = mkResources "500m" "512Mi" "2000m" "2Gi"
              }
      , service = baseService // { type = "LoadBalancer" }
      , autoscaling =
            baseAutoscaling
          //  { enabled = True
              , minReplicas = 3
              , maxReplicas = 20
              , targetCPUPercent = 65
              }
      , labels =
            mkLabels appName Environment.Production
          # [ { mapKey = "tier", mapValue = "frontend" }
            , { mapKey = "version", mapValue = "v0.3.19" }
            ]
      }

-- ============================================================
-- Task type for project tracking
-- ============================================================

let Task =
      { id : Natural
      , title : Text
      , status : Status
      , priority : Priority
      , tags : List Text
      }

let tasks
    : List Task
    = [ { id = 1
        , title = "Implement syntax highlighting"
        , status = Status.InProgress
        , priority = Priority.High
        , tags = [ "feature", "syntax" ]
        }
      , { id = 2
        , title = "Fix cursor blinking"
        , status = Status.Done
        , priority = Priority.Low
        , tags = [ "bug" ]
        }
      , { id = 3
        , title = "Add split view"
        , status = Status.Open
        , priority = Priority.Medium
        , tags = [ "feature", "ui" ]
        }
      ]

-- ============================================================
-- Output: all environment configurations
-- ============================================================

in  { development = developmentConfig
    , staging = stagingConfig
    , production = productionConfig
    , tasks
    , metadata =
      { version = "0.3.19"
      , generator = "dhall"
      , appName
      }
    }
