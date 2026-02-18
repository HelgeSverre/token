export interface ThemeData {
  id: string;
  name: string;
  author: string;
  description: string;
  fileName: string;
  isLight: boolean;
  colors: {
    bg: string;
    fg: string;
    currentLine: string;
    cursor: string;
    selection: string;
    gutterBg: string;
    gutterFg: string;
    sidebarBg: string;
    statusBg: string;
    statusFg: string;
    gutterFgActive: string;
    gutterBorder: string;
    tabBarBg: string;
    tabBarActiveBg: string;
    tabBarActiveFg: string;
    tabBarInactiveBg: string;
    tabBarInactiveFg: string;
    tabBarBorder: string;
    splitterBg: string;
    punctuation: string;
    keyword: string;
    function: string;
    string: string;
    number: string;
    comment: string;
    type: string;
    variable: string;
    operator: string;
    constant: string;
    tag: string;
    attribute: string;
  };
  yaml: string;
}

export const themes: ThemeData[] = [
  {
    id: "fleet-dark",
    name: "Fleet Dark",
    author: "Token",
    description: "Default dark theme inspired by JetBrains Fleet",
    fileName: "fleet-dark.yaml",
    isLight: false,
    colors: {
      bg: "#181818", fg: "#BCBEC4", currentLine: "#1E1E1E", cursor: "#FFFEF8",
      selection: "#214283", gutterBg: "#181818", gutterFg: "#606366",
      sidebarBg: "#181818", statusBg: "#1E1E1E", statusFg: "#787A80",
      gutterFgActive: "#BCBEC4", gutterBorder: "#2B2B2B",
      tabBarBg: "#1E1E1E", tabBarActiveBg: "#181818", tabBarActiveFg: "#FFFFFF",
      tabBarInactiveBg: "#252525", tabBarInactiveFg: "#808080", tabBarBorder: "#323232",
      splitterBg: "#252525", punctuation: "#A9B7C6",
      keyword: "#CC7832", function: "#FFC66D", string: "#6A8759",
      number: "#6897BB", comment: "#808080", type: "#CC7832",
      variable: "#BCBEC4", operator: "#A9B7C6", constant: "#6897BB",
      tag: "#E8BF6A", attribute: "#BABABA",
    },
    yaml: `version: 1
name: "Fleet Dark"
author: "Built-in"
description: "JetBrains Fleet-inspired dark theme"

ui:
  editor:
    background: "#181818"
    foreground: "#BCBEC4"
    current_line_background: "#1E1E1E"
    cursor_color: "#FFFEF8"
    selection_background: "#214283"
    secondary_cursor_color: "#FFFEF880"

  gutter:
    background: "#181818"
    foreground: "#606366"
    foreground_active: "#A1A3AB"
    border_color: "#2B2B2B"

  status_bar:
    background: "#1E1E1E"
    foreground: "#787A80"

  sidebar:
    background: "#181818"
    foreground: "#BCBEC4"
    selection_background: "#FFFFFF1A"
    selection_foreground: "#FFFFFF"
    hover_background: "#FFFFFF0D"
    folder_icon: "#FFC66D"
    file_icon: "#9CDCFE"
    border: "#2B2B2B"

  tab_bar:
    background: "#181818"
    active_background: "#1E1E1E"
    active_foreground: "#BCBEC4"
    inactive_background: "#232323"
    inactive_foreground: "#606366"
    border: "#2B2B2B"
    modified_indicator: "#BCBEC4"

  overlay:
    border: "#2B2B2B"
    background: "#181818E0"
    foreground: "#BCBEC4"
    input_background: "#0D0D0D"
    selection_background: "#214283"
    highlight: "#6AAF6A"
    warning: "#D9A74A"
    error: "#E05252"

  csv:
    header_background: "#1E1E1E"
    header_foreground: "#A1A3AB"
    grid_line: "#2B2B2B"
    selected_cell_background: "#21428380"
    selected_cell_border: "#4D78CC"
    number_foreground: "#6897BB"

  syntax:
    keyword: "#CC7832"
    function: "#FFC66D"
    function_builtin: "#FFC66D"
    string: "#6A8759"
    number: "#6897BB"
    comment: "#808080"
    type: "#CC7832"
    variable: "#BCBEC4"
    variable_builtin: "#94558D"
    property: "#9876AA"
    operator: "#A9B7C6"
    punctuation: "#A9B7C6"
    constant: "#6897BB"
    tag: "#E8BF6A"
    attribute: "#BABABA"
    escape: "#CC7832"
    label: "#BBB529"
    text: "#BCBEC4"
    text_emphasis: "#BCBEC4"
    text_strong: "#BCBEC4"
    text_title: "#FFC66D"
    text_uri: "#287BDE"`,
  },
  {
    id: "dark",
    name: "Default Dark",
    author: "Token",
    description: "VS Code-inspired dark theme",
    fileName: "dark.yaml",
    isLight: false,
    colors: {
      bg: "#1E1E1E", fg: "#D4D4D4", currentLine: "#2A2A2A", cursor: "#FCE146",
      selection: "#264F78", gutterBg: "#1E1E1E", gutterFg: "#858585",
      sidebarBg: "#252526", statusBg: "#007ACC", statusFg: "#FFFFFF",
      gutterFgActive: "#C6C6C6", gutterBorder: "#303030",
      tabBarBg: "#252526", tabBarActiveBg: "#1E1E1E", tabBarActiveFg: "#FFFFFF",
      tabBarInactiveBg: "#2D2D2D", tabBarInactiveFg: "#808080", tabBarBorder: "#3C3C3C",
      splitterBg: "#252526", punctuation: "#D4D4D4",
      keyword: "#C586C0", function: "#DCDCAA", string: "#CE9178",
      number: "#B5CEA8", comment: "#6A9955", type: "#4EC9B0",
      variable: "#9CDCFE", operator: "#D4D4D4", constant: "#569CD6",
      tag: "#569CD6", attribute: "#9CDCFE",
    },
    yaml: `version: 1
name: "Default Dark"
author: "Built-in"
description: "VS Code-inspired dark theme"

ui:
  editor:
    background: "#1E1E1E"
    foreground: "#D4D4D4"
    current_line_background: "#2A2A2A"
    cursor_color: "#FCE146"
    selection_background: "#264F78"
    secondary_cursor_color: "#FFFFFF80"

  gutter:
    background: "#1E1E1E"
    foreground: "#858585"
    foreground_active: "#C6C6C6"
    border_color: "#313438"

  status_bar:
    background: "#007ACC"
    foreground: "#FFFFFF"

  sidebar:
    background: "#252526"
    foreground: "#CCCCCC"
    selection_background: "#FFFFFF1A"
    selection_foreground: "#FFFFFF"
    hover_background: "#FFFFFF0D"
    folder_icon: "#DCDC8B"
    file_icon: "#9CDCFE"
    border: "#3C3C3C"

  tab_bar:
    background: "#252526"
    active_background: "#1E1E1E"
    active_foreground: "#FFFFFF"
    inactive_background: "#2D2D2D"
    inactive_foreground: "#808080"
    border: "#3C3C3C"
    modified_indicator: "#FFFFFF"

  overlay:
    border: "#43454A"
    background: "#2B2D30"
    foreground: "#E0E0E0"
    input_background: "#1E1E1E"
    selection_background: "#264F78"
    highlight: "#80FF80"
    warning: "#FFFF80"
    error: "#FF8080"

  csv:
    header_background: "#2D2D2D"
    header_foreground: "#E0E0E0"
    grid_line: "#404040"
    selected_cell_background: "#264F7880"
    selected_cell_border: "#007ACC"
    number_foreground: "#B5CEA8"

  syntax:
    keyword: "#C586C0"
    function: "#DCDCAA"
    function_builtin: "#DCDCAA"
    string: "#CE9178"
    number: "#B5CEA8"
    comment: "#6A9955"
    type: "#4EC9B0"
    variable: "#9CDCFE"
    variable_builtin: "#569CD6"
    property: "#9CDCFE"
    operator: "#D4D4D4"
    punctuation: "#D4D4D4"
    constant: "#569CD6"
    tag: "#569CD6"
    attribute: "#9CDCFE"
    escape: "#D7BA7D"
    label: "#D7BA7D"
    text: "#D4D4D4"
    text_emphasis: "#D4D4D4"
    text_strong: "#D4D4D4"
    text_title: "#569CD6"
    text_uri: "#3E9CD6"`,
  },
  {
    id: "github-dark",
    name: "GitHub Dark",
    author: "Token",
    description: "GitHub's dark theme colors",
    fileName: "github-dark.yaml",
    isLight: false,
    colors: {
      bg: "#0D1117", fg: "#C9D1D9", currentLine: "#161B22", cursor: "#58A6FF",
      selection: "#264F78", gutterBg: "#0D1117", gutterFg: "#484F58",
      sidebarBg: "#010409", statusBg: "#161B22", statusFg: "#8B949E",
      gutterFgActive: "#C9D1D9", gutterBorder: "#21262D",
      tabBarBg: "#161B22", tabBarActiveBg: "#0D1117", tabBarActiveFg: "#E6EDF3",
      tabBarInactiveBg: "#1C2128", tabBarInactiveFg: "#8B949E", tabBarBorder: "#21262D",
      splitterBg: "#21262D", punctuation: "#C9D1D9",
      keyword: "#FF7B72", function: "#D2A8FF", string: "#A5D6FF",
      number: "#79C0FF", comment: "#8B949E", type: "#FFA657",
      variable: "#C9D1D9", operator: "#C9D1D9", constant: "#79C0FF",
      tag: "#7EE787", attribute: "#79C0FF",
    },
    yaml: `version: 1
name: "GitHub Dark"
author: "Built-in"
description: "GitHub's dark theme"

ui:
  editor:
    background: "#0D1117"
    foreground: "#C9D1D9"
    current_line_background: "#161B22"
    cursor_color: "#58A6FF"
    selection_background: "#264F78"
    secondary_cursor_color: "#58A6FF80"

  gutter:
    background: "#0D1117"
    foreground: "#484F58"
    foreground_active: "#C9D1D9"
    border_color: "#21262D"

  status_bar:
    background: "#161B22"
    foreground: "#8B949E"

  sidebar:
    background: "#010409"
    foreground: "#C9D1D9"
    selection_background: "#FFFFFF1A"
    selection_foreground: "#FFFFFF"
    hover_background: "#FFFFFF0D"
    folder_icon: "#FFA657"
    file_icon: "#79C0FF"
    border: "#21262D"

  tab_bar:
    background: "#010409"
    active_background: "#0D1117"
    active_foreground: "#C9D1D9"
    inactive_background: "#161B22"
    inactive_foreground: "#484F58"
    border: "#21262D"
    modified_indicator: "#C9D1D9"

  overlay:
    border: "#30363D"
    background: "#0D1117E0"
    foreground: "#C9D1D9"
    input_background: "#010409"
    selection_background: "#264F78"
    highlight: "#3FB950"
    warning: "#D29922"
    error: "#F85149"

  csv:
    header_background: "#161B22"
    header_foreground: "#C9D1D9"
    grid_line: "#21262D"
    selected_cell_background: "#264F7880"
    selected_cell_border: "#58A6FF"
    number_foreground: "#79C0FF"

  syntax:
    keyword: "#FF7B72"
    function: "#D2A8FF"
    function_builtin: "#D2A8FF"
    string: "#A5D6FF"
    number: "#79C0FF"
    comment: "#8B949E"
    type: "#FFA657"
    variable: "#C9D1D9"
    variable_builtin: "#FFA657"
    property: "#79C0FF"
    operator: "#C9D1D9"
    punctuation: "#C9D1D9"
    constant: "#79C0FF"
    tag: "#7EE787"
    attribute: "#79C0FF"
    escape: "#FF7B72"
    label: "#FFA657"
    text: "#C9D1D9"
    text_emphasis: "#C9D1D9"
    text_strong: "#C9D1D9"
    text_title: "#58A6FF"
    text_uri: "#58A6FF"`,
  },
  {
    id: "github-light",
    name: "GitHub Light",
    author: "Token",
    description: "GitHub's clean light theme",
    fileName: "github-light.yaml",
    isLight: true,
    colors: {
      bg: "#FFFFFF", fg: "#24292F", currentLine: "#F6F8FA", cursor: "#0969DA",
      selection: "#ADD6FF", gutterBg: "#FFFFFF", gutterFg: "#8C959F",
      sidebarBg: "#F6F8FA", statusBg: "#F6F8FA", statusFg: "#57606A",
      gutterFgActive: "#24292F", gutterBorder: "#D0D7DE",
      tabBarBg: "#F6F8FA", tabBarActiveBg: "#FFFFFF", tabBarActiveFg: "#24292F",
      tabBarInactiveBg: "#EEF1F4", tabBarInactiveFg: "#57606A", tabBarBorder: "#D0D7DE",
      splitterBg: "#D0D7DE", punctuation: "#24292F",
      keyword: "#CF222E", function: "#8250DF", string: "#0A3069",
      number: "#0550AE", comment: "#6E7781", type: "#953800",
      variable: "#24292F", operator: "#24292F", constant: "#0550AE",
      tag: "#116329", attribute: "#0550AE",
    },
    yaml: `version: 1
name: "GitHub Light"
author: "Built-in"
description: "GitHub's light theme"

ui:
  editor:
    background: "#FFFFFF"
    foreground: "#24292F"
    current_line_background: "#F6F8FA"
    cursor_color: "#0969DA"
    selection_background: "#ADD6FF"
    secondary_cursor_color: "#0969DA80"

  gutter:
    background: "#FFFFFF"
    foreground: "#8C959F"
    foreground_active: "#24292F"
    border_color: "#D0D7DE"

  status_bar:
    background: "#F6F8FA"
    foreground: "#57606A"

  sidebar:
    background: "#F6F8FA"
    foreground: "#24292F"
    selection_background: "#0000001A"
    selection_foreground: "#24292F"
    hover_background: "#0000000D"
    folder_icon: "#953800"
    file_icon: "#0550AE"
    border: "#D0D7DE"

  tab_bar:
    background: "#F6F8FA"
    active_background: "#FFFFFF"
    active_foreground: "#24292F"
    inactive_background: "#EAEEF2"
    inactive_foreground: "#57606A"
    border: "#D0D7DE"
    modified_indicator: "#24292F"

  overlay:
    border: "#D0D7DE"
    background: "#F0F0F0E0"
    foreground: "#24292F"
    input_background: "#FFFFFF"
    selection_background: "#ADD6FF"
    highlight: "#1A7F37"
    warning: "#9A6700"
    error: "#CF222E"

  csv:
    header_background: "#F6F8FA"
    header_foreground: "#24292F"
    grid_line: "#D0D7DE"
    selected_cell_background: "#ADD6FF80"
    selected_cell_border: "#0969DA"
    number_foreground: "#0550AE"

  syntax:
    keyword: "#CF222E"
    function: "#8250DF"
    function_builtin: "#8250DF"
    string: "#0A3069"
    number: "#0550AE"
    comment: "#6E7781"
    type: "#953800"
    variable: "#24292F"
    variable_builtin: "#953800"
    property: "#0550AE"
    operator: "#24292F"
    punctuation: "#24292F"
    constant: "#0550AE"
    tag: "#116329"
    attribute: "#0550AE"
    escape: "#CF222E"
    label: "#953800"
    text: "#24292F"
    text_emphasis: "#24292F"
    text_strong: "#24292F"
    text_title: "#0969DA"
    text_uri: "#0969DA"`,
  },
  {
    id: "dracula",
    name: "Dracula",
    author: "Token",
    description: "The iconic dark theme with bold purples, pinks, and cyans",
    fileName: "dracula.yaml",
    isLight: false,
    colors: {
      bg: "#282A36", fg: "#F8F8F2", currentLine: "#44475A", cursor: "#F8F8F2",
      selection: "#44475A", gutterBg: "#282A36", gutterFg: "#6272A4",
      sidebarBg: "#21222C", statusBg: "#191A21", statusFg: "#F8F8F2",
      gutterFgActive: "#F8F8F2", gutterBorder: "#343746",
      tabBarBg: "#21222C", tabBarActiveBg: "#282A36", tabBarActiveFg: "#F8F8F2",
      tabBarInactiveBg: "#21222C", tabBarInactiveFg: "#6272A4", tabBarBorder: "#343746",
      splitterBg: "#343746", punctuation: "#F8F8F2",
      keyword: "#FF79C6", function: "#50FA7B", string: "#F1FA8C",
      number: "#BD93F9", comment: "#6272A4", type: "#8BE9FD",
      variable: "#F8F8F2", operator: "#FF79C6", constant: "#BD93F9",
      tag: "#FF79C6", attribute: "#50FA7B",
    },
    yaml: `version: 1
name: "Dracula"
author: "Built-in"
description: "The iconic dark theme with bold purples, pinks, and cyans"

ui:
  editor:
    background: "#282A36"
    foreground: "#F8F8F2"
    current_line_background: "#44475A"
    cursor_color: "#F8F8F2"
    selection_background: "#44475A"
    secondary_cursor_color: "#F8F8F280"

  gutter:
    background: "#282A36"
    foreground: "#6272A4"
    foreground_active: "#F8F8F2"
    border_color: "#343746"

  status_bar:
    background: "#191A21"
    foreground: "#F8F8F2"

  sidebar:
    background: "#21222C"
    foreground: "#F8F8F2"
    selection_background: "#44475A"
    selection_foreground: "#F8F8F2"
    hover_background: "#FFFFFF0D"
    folder_icon: "#F1FA8C"
    file_icon: "#8BE9FD"
    border: "#343746"

  tab_bar:
    background: "#21222C"
    active_background: "#282A36"
    active_foreground: "#F8F8F2"
    inactive_background: "#21222C"
    inactive_foreground: "#6272A4"
    border: "#343746"
    modified_indicator: "#F8F8F2"

  overlay:
    border: "#6272A4"
    background: "#282A36"
    foreground: "#F8F8F2"
    input_background: "#21222C"
    selection_background: "#44475A"
    highlight: "#50FA7B"
    warning: "#F1FA8C"
    error: "#FF5555"

  csv:
    header_background: "#21222C"
    header_foreground: "#F8F8F2"
    grid_line: "#44475A"
    selected_cell_background: "#44475A80"
    selected_cell_border: "#BD93F9"
    number_foreground: "#BD93F9"

  syntax:
    keyword: "#FF79C6"
    function: "#50FA7B"
    function_builtin: "#8BE9FD"
    string: "#F1FA8C"
    number: "#BD93F9"
    comment: "#6272A4"
    type: "#8BE9FD"
    variable: "#F8F8F2"
    variable_builtin: "#BD93F9"
    property: "#F8F8F2"
    operator: "#FF79C6"
    punctuation: "#F8F8F2"
    constant: "#BD93F9"
    tag: "#FF79C6"
    attribute: "#50FA7B"
    escape: "#FF79C6"
    label: "#8BE9FD"
    text: "#F8F8F2"
    text_emphasis: "#F1FA8C"
    text_strong: "#FFB86C"
    text_title: "#BD93F9"
    text_uri: "#8BE9FD"`,
  },
  {
    id: "mocha",
    name: "Catppuccin Mocha",
    author: "Token",
    description: "Soothing pastel theme with warm, cozy colors",
    fileName: "mocha.yaml",
    isLight: false,
    colors: {
      bg: "#1E1E2E", fg: "#CDD6F4", currentLine: "#28283D", cursor: "#F5E0DC",
      selection: "#45475A", gutterBg: "#1E1E2E", gutterFg: "#6C7086",
      sidebarBg: "#181825", statusBg: "#181825", statusFg: "#BAC2DE",
      gutterFgActive: "#CDD6F4", gutterBorder: "#313244",
      tabBarBg: "#181825", tabBarActiveBg: "#1E1E2E", tabBarActiveFg: "#CDD6F4",
      tabBarInactiveBg: "#181825", tabBarInactiveFg: "#6C7086", tabBarBorder: "#313244",
      splitterBg: "#313244", punctuation: "#BAC2DE",
      keyword: "#CBA6F7", function: "#89B4FA", string: "#A6E3A1",
      number: "#FAB387", comment: "#6C7086", type: "#F9E2AF",
      variable: "#CDD6F4", operator: "#94E2D5", constant: "#FAB387",
      tag: "#CBA6F7", attribute: "#89DCEB",
    },
    yaml: `version: 1
name: "Catppuccin Mocha"
author: "Built-in"
description: "Soothing pastel theme with warm, cozy colors"

ui:
  editor:
    background: "#1E1E2E"
    foreground: "#CDD6F4"
    current_line_background: "#28283D"
    cursor_color: "#F5E0DC"
    selection_background: "#45475A"
    secondary_cursor_color: "#F5E0DC80"

  gutter:
    background: "#1E1E2E"
    foreground: "#6C7086"
    foreground_active: "#CDD6F4"
    border_color: "#313244"

  status_bar:
    background: "#181825"
    foreground: "#BAC2DE"

  sidebar:
    background: "#181825"
    foreground: "#CDD6F4"
    selection_background: "#45475A"
    selection_foreground: "#CDD6F4"
    hover_background: "#FFFFFF0D"
    folder_icon: "#F9E2AF"
    file_icon: "#89B4FA"
    border: "#313244"

  tab_bar:
    background: "#181825"
    active_background: "#1E1E2E"
    active_foreground: "#CDD6F4"
    inactive_background: "#181825"
    inactive_foreground: "#6C7086"
    border: "#313244"
    modified_indicator: "#CDD6F4"

  overlay:
    border: "#45475A"
    background: "#1E1E2E"
    foreground: "#CDD6F4"
    input_background: "#181825"
    selection_background: "#45475A"
    highlight: "#A6E3A1"
    warning: "#F9E2AF"
    error: "#F38BA8"

  csv:
    header_background: "#181825"
    header_foreground: "#CDD6F4"
    grid_line: "#313244"
    selected_cell_background: "#45475A80"
    selected_cell_border: "#89B4FA"
    number_foreground: "#FAB387"

  syntax:
    keyword: "#CBA6F7"
    function: "#89B4FA"
    function_builtin: "#89B4FA"
    string: "#A6E3A1"
    number: "#FAB387"
    comment: "#6C7086"
    type: "#F9E2AF"
    variable: "#CDD6F4"
    variable_builtin: "#F38BA8"
    property: "#89DCEB"
    operator: "#94E2D5"
    punctuation: "#BAC2DE"
    constant: "#FAB387"
    tag: "#CBA6F7"
    attribute: "#89DCEB"
    escape: "#F2CDCD"
    label: "#F9E2AF"
    text: "#CDD6F4"
    text_emphasis: "#CDD6F4"
    text_strong: "#CDD6F4"
    text_title: "#89B4FA"
    text_uri: "#F5C2E7"`,
  },
  {
    id: "nord",
    name: "Nord",
    author: "Token",
    description: "Arctic, north-bluish color palette with dimmed pastels",
    fileName: "nord.yaml",
    isLight: false,
    colors: {
      bg: "#2E3440", fg: "#D8DEE9", currentLine: "#3B4252", cursor: "#D8DEE9",
      selection: "#434C5E", gutterBg: "#2E3440", gutterFg: "#4C566A",
      sidebarBg: "#2E3440", statusBg: "#3B4252", statusFg: "#D8DEE9",
      gutterFgActive: "#D8DEE9", gutterBorder: "#3B4252",
      tabBarBg: "#2E3440", tabBarActiveBg: "#3B4252", tabBarActiveFg: "#ECEFF4",
      tabBarInactiveBg: "#2E3440", tabBarInactiveFg: "#4C566A", tabBarBorder: "#3B4252",
      splitterBg: "#3B4252", punctuation: "#ECEFF4",
      keyword: "#81A1C1", function: "#88C0D0", string: "#A3BE8C",
      number: "#B48EAD", comment: "#4C566A", type: "#8FBCBB",
      variable: "#D8DEE9", operator: "#81A1C1", constant: "#5E81AC",
      tag: "#81A1C1", attribute: "#8FBCBB",
    },
    yaml: `version: 1
name: "Nord"
author: "Built-in"
description: "Arctic, north-bluish color palette with dimmed pastels"

ui:
  editor:
    background: "#2E3440"
    foreground: "#D8DEE9"
    current_line_background: "#3B4252"
    cursor_color: "#D8DEE9"
    selection_background: "#434C5E"
    secondary_cursor_color: "#D8DEE980"

  gutter:
    background: "#2E3440"
    foreground: "#4C566A"
    foreground_active: "#D8DEE9"
    border_color: "#3B4252"

  status_bar:
    background: "#3B4252"
    foreground: "#D8DEE9"

  sidebar:
    background: "#2E3440"
    foreground: "#D8DEE9"
    selection_background: "#3B4252"
    selection_foreground: "#ECEFF4"
    hover_background: "#FFFFFF0D"
    folder_icon: "#EBCB8B"
    file_icon: "#81A1C1"
    border: "#3B4252"

  tab_bar:
    background: "#2E3440"
    active_background: "#3B4252"
    active_foreground: "#ECEFF4"
    inactive_background: "#2E3440"
    inactive_foreground: "#4C566A"
    border: "#3B4252"
    modified_indicator: "#ECEFF4"

  overlay:
    border: "#4C566A"
    background: "#3B4252"
    foreground: "#D8DEE9"
    input_background: "#2E3440"
    selection_background: "#434C5E"
    highlight: "#A3BE8C"
    warning: "#EBCB8B"
    error: "#BF616A"

  csv:
    header_background: "#3B4252"
    header_foreground: "#D8DEE9"
    grid_line: "#434C5E"
    selected_cell_background: "#434C5E80"
    selected_cell_border: "#88C0D0"
    number_foreground: "#B48EAD"

  syntax:
    keyword: "#81A1C1"
    function: "#88C0D0"
    function_builtin: "#88C0D0"
    string: "#A3BE8C"
    number: "#B48EAD"
    comment: "#4C566A"
    type: "#8FBCBB"
    variable: "#D8DEE9"
    variable_builtin: "#81A1C1"
    property: "#D8DEE9"
    operator: "#81A1C1"
    punctuation: "#ECEFF4"
    constant: "#5E81AC"
    tag: "#81A1C1"
    attribute: "#8FBCBB"
    escape: "#EBCB8B"
    label: "#D08770"
    text: "#D8DEE9"
    text_emphasis: "#D8DEE9"
    text_strong: "#D8DEE9"
    text_title: "#88C0D0"
    text_uri: "#5E81AC"`,
  },
  {
    id: "tokyo-night",
    name: "Tokyo Night",
    author: "Token",
    description: "Neon lights of downtown Tokyo at night",
    fileName: "tokyo-night.yaml",
    isLight: false,
    colors: {
      bg: "#1A1B26", fg: "#A9B1D6", currentLine: "#24283B", cursor: "#C0CAF5",
      selection: "#33467C", gutterBg: "#1A1B26", gutterFg: "#3B4261",
      sidebarBg: "#15161E", statusBg: "#15161E", statusFg: "#7982A9",
      gutterFgActive: "#A9B1D6", gutterBorder: "#24283B",
      tabBarBg: "#15161E", tabBarActiveBg: "#1A1B26", tabBarActiveFg: "#C0CAF5",
      tabBarInactiveBg: "#15161E", tabBarInactiveFg: "#565F89", tabBarBorder: "#24283B",
      splitterBg: "#24283B", punctuation: "#A9B1D6",
      keyword: "#F7768E", function: "#7AA2F7", string: "#9ECE6A",
      number: "#FF9E64", comment: "#565F89", type: "#E0AF68",
      variable: "#C0CAF5", operator: "#89DDFF", constant: "#FF9E64",
      tag: "#F7768E", attribute: "#7DCFFF",
    },
    yaml: `version: 1
name: "Tokyo Night"
author: "Built-in"
description: "Neon lights of downtown Tokyo at night"

ui:
  editor:
    background: "#1A1B26"
    foreground: "#A9B1D6"
    current_line_background: "#24283B"
    cursor_color: "#C0CAF5"
    selection_background: "#33467C"
    secondary_cursor_color: "#C0CAF580"

  gutter:
    background: "#1A1B26"
    foreground: "#3B4261"
    foreground_active: "#A9B1D6"
    border_color: "#24283B"

  status_bar:
    background: "#15161E"
    foreground: "#7982A9"

  sidebar:
    background: "#15161E"
    foreground: "#A9B1D6"
    selection_background: "#33467C"
    selection_foreground: "#C0CAF5"
    hover_background: "#FFFFFF0D"
    folder_icon: "#E0AF68"
    file_icon: "#7AA2F7"
    border: "#24283B"

  tab_bar:
    background: "#15161E"
    active_background: "#1A1B26"
    active_foreground: "#C0CAF5"
    inactive_background: "#15161E"
    inactive_foreground: "#565F89"
    border: "#24283B"
    modified_indicator: "#C0CAF5"

  overlay:
    border: "#3B4261"
    background: "#1A1B26"
    foreground: "#C0CAF5"
    input_background: "#15161E"
    selection_background: "#33467C"
    highlight: "#9ECE6A"
    warning: "#E0AF68"
    error: "#F7768E"

  csv:
    header_background: "#24283B"
    header_foreground: "#C0CAF5"
    grid_line: "#3B4261"
    selected_cell_background: "#33467C80"
    selected_cell_border: "#7AA2F7"
    number_foreground: "#FF9E64"

  syntax:
    keyword: "#F7768E"
    function: "#7AA2F7"
    function_builtin: "#7AA2F7"
    string: "#9ECE6A"
    number: "#FF9E64"
    comment: "#565F89"
    type: "#E0AF68"
    variable: "#C0CAF5"
    variable_builtin: "#F7768E"
    property: "#7DCFFF"
    operator: "#89DDFF"
    punctuation: "#A9B1D6"
    constant: "#FF9E64"
    tag: "#F7768E"
    attribute: "#7DCFFF"
    escape: "#BB9AF7"
    label: "#E0AF68"
    text: "#A9B1D6"
    text_emphasis: "#A9B1D6"
    text_strong: "#A9B1D6"
    text_title: "#7AA2F7"
    text_uri: "#7DCFFF"`,
  },
  {
    id: "gruvbox-dark",
    name: "Gruvbox Dark",
    author: "Token",
    description: "Retro groove color scheme with warm, earthy tones",
    fileName: "gruvbox-dark.yaml",
    isLight: false,
    colors: {
      bg: "#282828", fg: "#EBDBB2", currentLine: "#32302F", cursor: "#EBDBB2",
      selection: "#504945", gutterBg: "#282828", gutterFg: "#665C54",
      sidebarBg: "#1D2021", statusBg: "#504945", statusFg: "#EBDBB2",
      gutterFgActive: "#EBDBB2", gutterBorder: "#3C3836",
      tabBarBg: "#1D2021", tabBarActiveBg: "#282828", tabBarActiveFg: "#EBDBB2",
      tabBarInactiveBg: "#1D2021", tabBarInactiveFg: "#928374", tabBarBorder: "#3C3836",
      splitterBg: "#3C3836", punctuation: "#EBDBB2",
      keyword: "#FB4934", function: "#FABD2F", string: "#B8BB26",
      number: "#D3869B", comment: "#928374", type: "#83A598",
      variable: "#EBDBB2", operator: "#EBDBB2", constant: "#D3869B",
      tag: "#FB4934", attribute: "#B8BB26",
    },
    yaml: `version: 1
name: "Gruvbox Dark"
author: "Built-in"
description: "Retro groove color scheme with warm, earthy tones"

ui:
  editor:
    background: "#282828"
    foreground: "#EBDBB2"
    current_line_background: "#32302F"
    cursor_color: "#EBDBB2"
    selection_background: "#504945"
    secondary_cursor_color: "#EBDBB280"

  gutter:
    background: "#282828"
    foreground: "#665C54"
    foreground_active: "#EBDBB2"
    border_color: "#3C3836"

  status_bar:
    background: "#504945"
    foreground: "#EBDBB2"

  sidebar:
    background: "#1D2021"
    foreground: "#EBDBB2"
    selection_background: "#3C3836"
    selection_foreground: "#EBDBB2"
    hover_background: "#FFFFFF0D"
    folder_icon: "#FABD2F"
    file_icon: "#83A598"
    border: "#3C3836"

  tab_bar:
    background: "#1D2021"
    active_background: "#282828"
    active_foreground: "#EBDBB2"
    inactive_background: "#1D2021"
    inactive_foreground: "#928374"
    border: "#3C3836"
    modified_indicator: "#EBDBB2"

  overlay:
    border: "#504945"
    background: "#282828"
    foreground: "#EBDBB2"
    input_background: "#1D2021"
    selection_background: "#504945"
    highlight: "#B8BB26"
    warning: "#FABD2F"
    error: "#FB4934"

  csv:
    header_background: "#3C3836"
    header_foreground: "#EBDBB2"
    grid_line: "#504945"
    selected_cell_background: "#50494580"
    selected_cell_border: "#FABD2F"
    number_foreground: "#D3869B"

  syntax:
    keyword: "#FB4934"
    function: "#FABD2F"
    function_builtin: "#FABD2F"
    string: "#B8BB26"
    number: "#D3869B"
    comment: "#928374"
    type: "#83A598"
    variable: "#EBDBB2"
    variable_builtin: "#FE8019"
    property: "#83A598"
    operator: "#EBDBB2"
    punctuation: "#EBDBB2"
    constant: "#D3869B"
    tag: "#FB4934"
    attribute: "#B8BB26"
    escape: "#FE8019"
    label: "#FABD2F"
    text: "#EBDBB2"
    text_emphasis: "#EBDBB2"
    text_strong: "#EBDBB2"
    text_title: "#FABD2F"
    text_uri: "#83A598"`,
  },
];
