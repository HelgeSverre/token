# Terraform/HCL Syntax Highlighting Test
# Infrastructure for a containerized web application on AWS.

terraform {
  required_version = ">= 1.5.0"

  required_providers {
    aws = {
      source  = "hashicorp/aws"
      version = "~> 5.0"
    }
    random = {
      source  = "hashicorp/random"
      version = "~> 3.5"
    }
  }

  backend "s3" {
    bucket         = "myapp-terraform-state"
    key            = "prod/terraform.tfstate"
    region         = "us-east-1"
    encrypt        = true
    dynamodb_table = "terraform-locks"
  }
}

# Provider configuration
provider "aws" {
  region = var.aws_region

  default_tags {
    tags = {
      Project     = var.project_name
      Environment = var.environment
      ManagedBy   = "terraform"
    }
  }
}

# ============================================================
# Variables
# ============================================================

variable "project_name" {
  description = "Name of the project"
  type        = string
  default     = "myapp"
}

variable "environment" {
  description = "Deployment environment"
  type        = string
  default     = "production"

  validation {
    condition     = contains(["development", "staging", "production"], var.environment)
    error_message = "Environment must be development, staging, or production."
  }
}

variable "aws_region" {
  type    = string
  default = "us-east-1"
}

variable "container_config" {
  description = "Container configuration"
  type = object({
    image       = string
    cpu         = number
    memory      = number
    port        = number
    replicas    = number
    environment = map(string)
    health_check = optional(object({
      path     = string
      interval = number
    }), {
      path     = "/health"
      interval = 30
    })
  })

  default = {
    image    = "myapp:latest"
    cpu      = 256
    memory   = 512
    port     = 8080
    replicas = 2
    environment = {
      NODE_ENV  = "production"
      LOG_LEVEL = "info"
    }
  }
}

variable "enable_cdn" {
  type    = bool
  default = true
}

variable "allowed_cidrs" {
  type    = list(string)
  default = ["0.0.0.0/0"]
}

variable "db_config" {
  type = object({
    instance_class = string
    engine_version = string
    multi_az       = bool
    storage_gb     = number
  })
  default = {
    instance_class = "db.t3.medium"
    engine_version = "15.4"
    multi_az       = true
    storage_gb     = 100
  }
  sensitive = false
}

# ============================================================
# Locals
# ============================================================

locals {
  name_prefix = "${var.project_name}-${var.environment}"
  common_tags = {
    Project     = var.project_name
    Environment = var.environment
  }

  azs = slice(data.aws_availability_zones.available.names, 0, 3)

  private_subnets = [for i, az in local.azs : cidrsubnet("10.0.0.0/16", 8, i)]
  public_subnets  = [for i, az in local.azs : cidrsubnet("10.0.0.0/16", 8, i + 100)]

  container_env = [
    for key, value in var.container_config.environment : {
      name  = key
      value = value
    }
  ]
}

# ============================================================
# Data Sources
# ============================================================

data "aws_availability_zones" "available" {
  state = "available"
}

data "aws_caller_identity" "current" {}

data "aws_ecr_repository" "app" {
  name = var.project_name
}

# ============================================================
# VPC & Networking
# ============================================================

resource "aws_vpc" "main" {
  cidr_block           = "10.0.0.0/16"
  enable_dns_hostnames = true
  enable_dns_support   = true

  tags = {
    Name = "${local.name_prefix}-vpc"
  }
}

resource "aws_subnet" "private" {
  count = length(local.azs)

  vpc_id            = aws_vpc.main.id
  cidr_block        = local.private_subnets[count.index]
  availability_zone = local.azs[count.index]

  tags = {
    Name = "${local.name_prefix}-private-${local.azs[count.index]}"
    Tier = "private"
  }
}

resource "aws_subnet" "public" {
  count = length(local.azs)

  vpc_id                  = aws_vpc.main.id
  cidr_block              = local.public_subnets[count.index]
  availability_zone       = local.azs[count.index]
  map_public_ip_on_launch = true

  tags = {
    Name = "${local.name_prefix}-public-${local.azs[count.index]}"
    Tier = "public"
  }
}

resource "aws_internet_gateway" "main" {
  vpc_id = aws_vpc.main.id
  tags   = { Name = "${local.name_prefix}-igw" }
}

resource "aws_security_group" "alb" {
  name_prefix = "${local.name_prefix}-alb-"
  vpc_id      = aws_vpc.main.id
  description = "Security group for ALB"

  ingress {
    protocol    = "tcp"
    from_port   = 443
    to_port     = 443
    cidr_blocks = var.allowed_cidrs
    description = "HTTPS"
  }

  ingress {
    protocol    = "tcp"
    from_port   = 80
    to_port     = 80
    cidr_blocks = var.allowed_cidrs
    description = "HTTP (redirect to HTTPS)"
  }

  egress {
    protocol    = "-1"
    from_port   = 0
    to_port     = 0
    cidr_blocks = ["0.0.0.0/0"]
  }

  lifecycle {
    create_before_destroy = true
  }
}

# ============================================================
# ECS Cluster & Service
# ============================================================

resource "aws_ecs_cluster" "main" {
  name = "${local.name_prefix}-cluster"

  setting {
    name  = "containerInsights"
    value = "enabled"
  }
}

resource "aws_ecs_task_definition" "app" {
  family                   = "${local.name_prefix}-app"
  requires_compatibilities = ["FARGATE"]
  network_mode             = "awsvpc"
  cpu                      = var.container_config.cpu
  memory                   = var.container_config.memory
  execution_role_arn       = aws_iam_role.ecs_execution.arn
  task_role_arn            = aws_iam_role.ecs_task.arn

  container_definitions = jsonencode([
    {
      name      = "app"
      image     = "${data.aws_ecr_repository.app.repository_url}:${var.container_config.image}"
      essential = true

      portMappings = [
        {
          containerPort = var.container_config.port
          hostPort      = var.container_config.port
          protocol      = "tcp"
        }
      ]

      environment = local.container_env

      logConfiguration = {
        logDriver = "awslogs"
        options = {
          "awslogs-group"         = aws_cloudwatch_log_group.app.name
          "awslogs-region"        = var.aws_region
          "awslogs-stream-prefix" = "ecs"
        }
      }

      healthCheck = {
        command     = ["CMD-SHELL", "curl -f http://localhost:${var.container_config.port}${var.container_config.health_check.path} || exit 1"]
        interval    = var.container_config.health_check.interval
        timeout     = 5
        retries     = 3
        startPeriod = 60
      }
    }
  ])

  tags = local.common_tags
}

resource "aws_ecs_service" "app" {
  name            = "${local.name_prefix}-service"
  cluster         = aws_ecs_cluster.main.id
  task_definition = aws_ecs_task_definition.app.arn
  desired_count   = var.container_config.replicas
  launch_type     = "FARGATE"

  network_configuration {
    subnets          = aws_subnet.private[*].id
    security_groups  = [aws_security_group.ecs_tasks.id]
    assign_public_ip = false
  }

  load_balancer {
    target_group_arn = aws_lb_target_group.app.arn
    container_name   = "app"
    container_port   = var.container_config.port
  }

  deployment_circuit_breaker {
    enable   = true
    rollback = true
  }

  depends_on = [aws_lb_listener.https]
}

# ============================================================
# RDS Database
# ============================================================

resource "random_password" "db" {
  length  = 32
  special = true
}

resource "aws_db_instance" "main" {
  identifier     = "${local.name_prefix}-db"
  engine         = "postgres"
  engine_version = var.db_config.engine_version
  instance_class = var.db_config.instance_class

  allocated_storage     = var.db_config.storage_gb
  max_allocated_storage = var.db_config.storage_gb * 2
  storage_encrypted     = true

  db_name  = replace(var.project_name, "-", "_")
  username = "admin"
  password = random_password.db.result

  multi_az               = var.db_config.multi_az
  db_subnet_group_name   = aws_db_subnet_group.main.name
  vpc_security_group_ids = [aws_security_group.db.id]

  backup_retention_period = 7
  skip_final_snapshot     = var.environment != "production"

  tags = local.common_tags
}

# ============================================================
# Outputs
# ============================================================

output "vpc_id" {
  description = "VPC ID"
  value       = aws_vpc.main.id
}

output "cluster_name" {
  description = "ECS cluster name"
  value       = aws_ecs_cluster.main.name
}

output "database_endpoint" {
  description = "RDS endpoint"
  value       = aws_db_instance.main.endpoint
  sensitive   = true
}

output "service_url" {
  description = "Application URL"
  value       = "https://${var.project_name}.example.com"
}
