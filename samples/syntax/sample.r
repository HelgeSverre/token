# R Syntax Highlighting Test
# Exploratory data analysis and visualization pipeline.

library(tidyverse)
library(lubridate)
library(scales)
library(glue)

# ============================================================
# Constants and configuration
# ============================================================

VERSION <- "1.0.0"
SEED <- 42
N_SAMPLES <- 1000
CONFIDENCE_LEVEL <- 0.95

theme_custom <- theme_minimal() +
  theme(
    plot.title = element_text(size = 16, face = "bold"),
    plot.subtitle = element_text(size = 12, color = "gray40"),
    axis.title = element_text(size = 11),
    legend.position = "bottom",
    panel.grid.minor = element_blank()
  )

color_palette <- c(
  primary   = "#3b82f6",
  secondary = "#6366f1",
  success   = "#22c55e",
  warning   = "#f59e0b",
  danger    = "#ef4444"
)

# ============================================================
# Data generation
# ============================================================

set.seed(SEED)

generate_sales_data <- function(n = N_SAMPLES,
                                 start_date = "2024-01-01",
                                 end_date = "2024-12-31") {
  dates <- seq(
    from = as.Date(start_date),
    to = as.Date(end_date),
    length.out = n
  )

  categories <- c("Electronics", "Clothing", "Books", "Food", "Sports")
  regions <- c("North", "South", "East", "West")

  tibble(
    date = sample(dates, n, replace = TRUE),
    category = sample(categories, n, replace = TRUE, prob = c(0.3, 0.25, 0.15, 0.2, 0.1)),
    region = sample(regions, n, replace = TRUE),
    units = rpois(n, lambda = 15),
    price = round(rlnorm(n, meanlog = 3.5, sdlog = 0.8), 2),
    discount = rbeta(n, shape1 = 2, shape2 = 8),
    returning_customer = rbinom(n, size = 1, prob = 0.35) == 1L
  ) %>%
    mutate(
      revenue = units * price * (1 - discount),
      month = floor_date(date, "month"),
      quarter = quarter(date),
      day_of_week = wday(date, label = TRUE, abbr = TRUE),
      is_weekend = day_of_week %in% c("Sat", "Sun")
    ) %>%
    arrange(date)
}

sales <- generate_sales_data()

# ============================================================
# Analysis functions
# ============================================================

# Summary statistics with confidence intervals
summarize_with_ci <- function(data, var, conf = CONFIDENCE_LEVEL) {
  var_sym <- ensym(var)

  data %>%
    summarize(
      n = n(),
      mean = mean({{ var_sym }}, na.rm = TRUE),
      sd = sd({{ var_sym }}, na.rm = TRUE),
      median = median({{ var_sym }}, na.rm = TRUE),
      q25 = quantile({{ var_sym }}, 0.25, na.rm = TRUE),
      q75 = quantile({{ var_sym }}, 0.75, na.rm = TRUE),
      .groups = "drop"
    ) %>%
    mutate(
      se = sd / sqrt(n),
      z = qnorm(1 - (1 - conf) / 2),
      ci_lower = mean - z * se,
      ci_upper = mean + z * se
    ) %>%
    select(-z)
}

# Cohort analysis
analyze_cohorts <- function(data) {
  data %>%
    group_by(category, region) %>%
    summarize_with_ci(revenue) %>%
    mutate(
      category = fct_reorder(category, mean, .desc = TRUE)
    )
}

# Time series decomposition
decompose_trend <- function(data, date_col, value_col) {
  monthly <- data %>%
    group_by(month = floor_date({{ date_col }}, "month")) %>%
    summarize(
      value = sum({{ value_col }}, na.rm = TRUE),
      count = n(),
      .groups = "drop"
    )

  # Rolling average
  monthly %>%
    mutate(
      trend = zoo::rollmean(value, k = 3, fill = NA, align = "center"),
      yoy_growth = (value / lag(value, 12) - 1) * 100,
      mom_growth = (value / lag(value, 1) - 1) * 100
    )
}

# ============================================================
# Visualizations
# ============================================================

# Revenue by category over time
plot_revenue_trend <- function(data) {
  monthly <- data %>%
    group_by(month, category) %>%
    summarize(revenue = sum(revenue), .groups = "drop")

  ggplot(monthly, aes(x = month, y = revenue, color = category, fill = category)) +
    geom_area(alpha = 0.15, position = "identity") +
    geom_line(linewidth = 0.8) +
    geom_point(size = 1.5) +
    scale_y_continuous(labels = dollar_format()) +
    scale_x_date(date_labels = "%b %Y", date_breaks = "2 months") +
    scale_color_brewer(palette = "Set2") +
    scale_fill_brewer(palette = "Set2") +
    labs(
      title = "Monthly Revenue by Category",
      subtitle = glue("Period: {min(data$date)} to {max(data$date)}"),
      x = NULL,
      y = "Revenue ($)",
      color = "Category",
      fill = "Category"
    ) +
    theme_custom
}

# Distribution comparison with violin + boxplot
plot_distribution <- function(data, var, group_var) {
  ggplot(data, aes(x = {{ group_var }}, y = {{ var }}, fill = {{ group_var }})) +
    geom_violin(alpha = 0.3, draw_quantiles = c(0.25, 0.5, 0.75)) +
    geom_boxplot(width = 0.15, alpha = 0.7, outlier.shape = 21) +
    stat_summary(fun = mean, geom = "point", shape = 18, size = 3, color = "red") +
    scale_fill_brewer(palette = "Pastel1") +
    coord_flip() +
    labs(
      title = glue("Distribution of {deparse(substitute(var))} by {deparse(substitute(group_var))}"),
      fill = NULL
    ) +
    theme_custom +
    theme(legend.position = "none")
}

# Correlation heatmap
plot_correlation <- function(data) {
  numeric_cols <- data %>%
    select(where(is.numeric)) %>%
    select(-quarter)

  cor_matrix <- cor(numeric_cols, use = "pairwise.complete.obs")

  cor_matrix %>%
    as.data.frame() %>%
    rownames_to_column("var1") %>%
    pivot_longer(-var1, names_to = "var2", values_to = "correlation") %>%
    ggplot(aes(x = var1, y = var2, fill = correlation)) +
    geom_tile() +
    geom_text(aes(label = round(correlation, 2)), size = 3) +
    scale_fill_gradient2(
      low = color_palette["danger"],
      mid = "white",
      high = color_palette["primary"],
      midpoint = 0,
      limits = c(-1, 1)
    ) +
    labs(title = "Correlation Matrix", x = NULL, y = NULL) +
    theme_custom +
    theme(axis.text.x = element_text(angle = 45, hjust = 1))
}

# ============================================================
# Statistical modeling
# ============================================================

# Linear model with formula interface
fit_revenue_model <- function(data) {
  model <- lm(
    revenue ~ category * region + units + price + discount +
      returning_customer + is_weekend + poly(as.numeric(date), 2),
    data = data
  )

  # Model diagnostics
  cat(glue("\n=== Revenue Model ===\n"))
  cat(glue("R² = {round(summary(model)$r.squared, 4)}\n"))
  cat(glue("Adjusted R² = {round(summary(model)$adj.r.squared, 4)}\n"))
  cat(glue("F-statistic p-value = {format.pval(pf(
    summary(model)$fstatistic[1],
    summary(model)$fstatistic[2],
    summary(model)$fstatistic[3],
    lower.tail = FALSE
  ))}\n\n"))

  # Tidy output
  broom::tidy(model, conf.int = TRUE) %>%
    filter(p.value < 0.05) %>%
    arrange(p.value) %>%
    mutate(
      significance = case_when(
        p.value < 0.001 ~ "***",
        p.value < 0.01  ~ "**",
        p.value < 0.05  ~ "*",
        TRUE             ~ ""
      )
    )
}

# ============================================================
# Report generation
# ============================================================

generate_report <- function(data, output_dir = "output") {
  if (!dir.exists(output_dir)) dir.create(output_dir, recursive = TRUE)

  # Key metrics
  metrics <- list(
    total_revenue = sum(data$revenue),
    avg_order = mean(data$revenue),
    total_units = sum(data$units),
    return_rate = mean(data$returning_customer),
    top_category = data %>%
      count(category, wt = revenue, sort = TRUE) %>%
      slice(1) %>%
      pull(category)
  )

  cat(glue("
    ╔══════════════════════════════════════╗
    ║         Sales Report Summary         ║
    ╠══════════════════════════════════════╣
    ║ Total Revenue:  ${dollar(metrics$total_revenue)}
    ║ Avg Order:      ${dollar(metrics$avg_order)}
    ║ Total Units:    ${comma(metrics$total_units)}
    ║ Return Rate:    ${percent(metrics$return_rate)}
    ║ Top Category:   {metrics$top_category}
    ╚══════════════════════════════════════╝
  "))

  # Save plots
  plots <- list(
    trend = plot_revenue_trend(data),
    dist = plot_distribution(data, revenue, category),
    corr = plot_correlation(data)
  )

  iwalk(plots, ~ {
    ggsave(
      filename = file.path(output_dir, glue("{.y}.png")),
      plot = .x,
      width = 10,
      height = 6,
      dpi = 300
    )
  })

  invisible(metrics)
}

# ============================================================
# Main execution
# ============================================================

if (sys.nframe() == 0) {
  cat(glue("Sales Analysis v{VERSION}\n\n"))

  # Run analysis
  cohorts <- analyze_cohorts(sales)
  trend <- decompose_trend(sales, date, revenue)
  model_results <- fit_revenue_model(sales)

  # Generate report
  report <- generate_report(sales)

  cat("\nAnalysis complete.\n")
}
