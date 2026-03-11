// CUE Syntax Highlighting Test
// A deployment configuration with types-as-values and constraints.

package deploy

import (
	"strings"
	"list"
	"time"
)

// ============================================================
// Type definitions (types are values in CUE)
// ============================================================

#Priority: "low" | "medium" | "high" | "critical"

#Status: "open" | "in_progress" | "done" | "cancelled"

#Environment: "development" | "staging" | "production"

#Port: int & >=1 & <=65535

#CPUAmount: =~"^[0-9]+(m)?$"

#MemoryAmount: =~"^[0-9]+(Mi|Gi)$"

#ImageTag: string & !="" & !=~"\\s"

// Resource constraints
#ResourceLimits: {
	cpu:    #CPUAmount
	memory: #MemoryAmount
}

#Resources: {
	requests: #ResourceLimits
	limits:   #ResourceLimits

	// Limits must be >= requests (basic validation)
	// CUE enforces this structurally
}

#HealthCheck: {
	path:                string & =~"^/"
	port:                #Port
	initialDelaySeconds: int & >=0 | *10
	periodSeconds:       int & >=1 | *30
	timeoutSeconds:      int & >=1 | *5
	failureThreshold:    int & >=1 | *3
}

#EnvVar: {
	name:    string & =~"^[A-Z][A-Z0-9_]*$"
	value:   string
	secret?: bool | *false
}

#ContainerConfig: {
	name:         string & =~"^[a-z][a-z0-9-]*$"
	image:        #ImageTag
	tag:          string | *"latest"
	ports:        [...#Port] & list.MinItems(1)
	env:          [...#EnvVar]
	resources:    #Resources
	healthCheck?: #HealthCheck
	command?:     [...string]

	// Computed fields
	fullImage: "\(image):\(tag)"
}

#ServiceConfig: {
	name:       string
	type:       "ClusterIP" | "NodePort" | "LoadBalancer" | *"ClusterIP"
	port:       #Port
	targetPort: #Port
}

#AutoscalingConfig: {
	enabled:          bool | *false
	minReplicas:      int & >=1 | *1
	maxReplicas:      int & >=minReplicas | *10
	targetCPUPercent: int & >=1 & <=100 | *70

	if !enabled {
		minReplicas: 1
		maxReplicas: 1
	}
}

// Full deployment definition
#Deployment: {
	environment: #Environment
	namespace:   string & =~"^[a-z][a-z0-9-]*$"
	replicas:    int & >=1

	container:   #ContainerConfig
	service:     #ServiceConfig
	autoscaling: #AutoscalingConfig

	labels: [string]: string
	labels: {
		app:         container.name
		environment: environment
		"managed-by": "cue"
	}

	// Cross-field validation
	if autoscaling.enabled {
		replicas: >=autoscaling.minReplicas
		replicas: <=autoscaling.maxReplicas
	}
}

// ============================================================
// Task tracking
// ============================================================

#Task: {
	id:           int & >0
	title:        string & strings.MinRunes(1)
	description?: string
	status:       #Status | *"open"
	priority:     #Priority | *"medium"
	tags:         [...string] | *[]
	assignee?:    string

	// Completed tasks must have done status
	if status == "done" {
		completedAt: string
	}
}

// ============================================================
// Shared defaults
// ============================================================

_appName: "token-editor-api"

_baseContainer: #ContainerConfig & {
	name:  _appName
	image: "ghcr.io/example/\(_appName)"
	ports: [8080, 9090]
	env: [
		{name: "APP_NAME", value:           "token-editor"},
		{name: "LOG_FORMAT", value:         "json"},
		{name: "METRICS_ENABLED", value:    "true"},
		{name: "DATABASE_URL", value:       "from-secret", secret: true},
	]
	resources: {
		requests: {cpu: "100m", memory: "128Mi"}
		limits: {cpu:   "500m", memory: "512Mi"}
	}
	healthCheck: {
		path: "/health"
		port: 8080
	}
}

_baseService: #ServiceConfig & {
	name:       _appName
	port:       80
	targetPort: 8080
}

// ============================================================
// Environment configurations
// ============================================================

deployments: [Name=string]: #Deployment

deployments: {
	development: {
		environment: "development"
		namespace:   "token-editor-dev"
		replicas:    1

		container: _baseContainer & {
			tag: "dev"
			env: _baseContainer.env + [
				{name: "ENVIRONMENT", value: "development"},
				{name: "LOG_LEVEL", value:   "debug"},
			]
			resources: {
				requests: {cpu: "50m", memory:  "64Mi"}
				limits: {cpu:   "200m", memory: "256Mi"}
			}
		}
		service:     _baseService
		autoscaling: enabled: false
	}

	staging: {
		environment: "staging"
		namespace:   "token-editor-staging"
		replicas:    2

		container: _baseContainer & {
			tag: "rc-latest"
			env: _baseContainer.env + [
				{name: "ENVIRONMENT", value: "staging"},
				{name: "LOG_LEVEL", value:   "info"},
			]
		}
		service: _baseService
		autoscaling: {
			enabled:     true
			minReplicas: 2
			maxReplicas: 5
		}
	}

	production: {
		environment: "production"
		namespace:   "token-editor-prod"
		replicas:    3

		container: _baseContainer & {
			tag: "v0.3.19"
			env: _baseContainer.env + [
				{name: "ENVIRONMENT", value: "production"},
				{name: "LOG_LEVEL", value:   "warn"},
				{name: "RATE_LIMIT", value:  "1000"},
				{name: "CACHE_TTL", value:   "300"},
			]
			resources: {
				requests: {cpu: "500m", memory:  "512Mi"}
				limits: {cpu:   "2000m", memory: "2Gi"}
			}
		}
		service: _baseService & {
			type: "LoadBalancer"
		}
		autoscaling: {
			enabled:          true
			minReplicas:      3
			maxReplicas:      20
			targetCPUPercent: 65
		}
		labels: {
			tier:    "frontend"
			version: container.tag
		}
	}
}

// ============================================================
// Tasks
// ============================================================

tasks: [...#Task] & [
	{
		id: 1, title: "Implement syntax highlighting"
		status: "in_progress", priority: "high"
		tags: ["feature", "syntax"]
	},
	{
		id: 2, title: "Fix cursor blinking"
		status: "done", priority: "low"
		tags: ["bug"]
		completedAt: "2024-12-15T10:00:00Z"
	},
	{
		id: 3, title: "Add split view"
		priority: "medium"
		tags: ["feature", "ui"]
	},
]

// ============================================================
// Computed outputs
// ============================================================

summary: {
	totalTasks:     len(tasks)
	environments:   len(deployments)
	productionImage: deployments.production.container.fullImage

	tasksByStatus: {
		for t in tasks {
			"\(t.status)": _ | *0
		}
		for t in tasks {
			"\(t.status)": _ + 1
		}
	}
}
