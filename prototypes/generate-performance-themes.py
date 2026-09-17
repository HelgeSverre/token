#!/usr/bin/env python3
"""Regenerate the offline prototype/component palette from Token's registry.

Run: python3 prototypes/generate-performance-themes.py (requires PyYAML).
Only overlay colors used by the visual studies need resolution; the formulas below
mirror src/theme.rs::resolve_overlay_theme. Explicit YAML values always win.
"""
import hashlib
import json
import math
from pathlib import Path
import re

import yaml

ROOT = Path(__file__).resolve().parents[1]


def rgb(color):
    return tuple(int(color[i:i + 2], 16) for i in (1, 3, 5))


def mix(a, b, amount):
    return "#" + "".join(f"{math.floor(x + (y - x) * amount + .5):02X}"
                          for x, y in zip(rgb(a), rgb(b)))


def luminance(color):
    def channel(value):
        value /= 255
        return value / 12.92 if value <= .03928 else ((value + .055) / 1.055) ** 2.4
    return sum(weight * channel(value) for weight, value in zip((.2126, .7152, .0722), rgb(color)))


def contrast(a, b):
    lo, hi = sorted((luminance(a), luminance(b)))
    return (hi + .05) / (lo + .05)


def ensure_contrast(foreground, background, pole, ratio):
    if contrast(foreground, background) >= ratio:
        return foreground
    lo, hi = 0, 1
    for _ in range(16):
        mid = (lo + hi) / 2
        if contrast(mix(foreground, pole, mid), background) >= ratio + .1:
            hi = mid
        else:
            lo = mid
    return mix(foreground, pole, hi)


def resolve_overlay(ui):
    o = dict(ui['overlay'])
    bg, fg = o['background'], o['foreground']
    light = luminance(bg) > .5
    away = '#000000' if light else '#FFFFFF'
    panel = o.setdefault('panel_background', bg)
    pole = max(('#FFFFFF', '#000000'), key=lambda c: contrast(c, panel))
    accent = o.setdefault('accent', ensure_contrast(ui['status_bar']['background'], panel, away, 3))
    o.setdefault('accent_bright', mix(accent, away, .3))
    o.setdefault('panel_secondary', mix(panel, fg if light else '#000000', .04 if light else .06))
    o.setdefault('recessed_wash', mix(panel, fg if light else '#000000', .06 if light else .15))
    o.setdefault('hairline', mix(panel, fg, .09))
    o.setdefault('selection_wash', mix(panel, accent, .28))
    o.setdefault('text_primary', ensure_contrast(fg, panel, pole, 7))
    o.setdefault('text_bright', pole)
    secondary = o.setdefault('text_secondary', ensure_contrast(mix(fg, panel, .35), panel, fg, 4.5))
    o.setdefault('text_dim', ensure_contrast(mix(secondary, panel, .35), panel, secondary, 4.5))
    o.setdefault('keycap_bg', mix(panel, fg, .08))
    o.setdefault('keycap_border', mix(panel, fg, .18))
    keycap_pole = max(('#FFFFFF', '#000000'), key=lambda c: contrast(c, o['keycap_bg']))
    o.setdefault('keycap_fg', ensure_contrast(fg, o['keycap_bg'], keycap_pole, 4.5))
    for role, seed in (('error', o['error']), ('warning', o['warning']), ('info', accent)):
        severity = o.setdefault('severity_' + role, ensure_contrast(seed, panel, pole, 4.5))
        ground = mix(panel, severity, .15)
        ground_pole = max(('#FFFFFF', '#000000'), key=lambda c: contrast(c, ground))
        o.setdefault('severity_' + role + '_text', ensure_contrast(severity, ground, ground_pole, 4.5))
    return o


# CSS token -> authoritative theme field. Custom-study defaults stay in HTML.
# Docks use sidebar colors, as DockPaneScene::resolve does in view/panels.rs.
FIELDS = {
    'desk': 'editor.background', 'editor': 'editor.background',
    'panel': 'sidebar.background', 'chrome': 'sidebar.background',
    'line': 'sidebar.border', 'soft-line': 'gutter.border_color',
    'text': 'sidebar.foreground', 'muted': 'overlay.text_secondary',
    'faint': 'overlay.text_dim', 'teal': 'overlay.highlight',
    'blue': 'syntax.type', 'amber': 'overlay.warning',
    'violet': 'syntax.keyword', 'red': 'overlay.error',
    'control-bg': 'overlay.keycap_bg', 'control-hover': 'overlay.panel_secondary',
    'control-selected': 'overlay.selection_background',
    'control-selected-fg': 'overlay.text_bright', 'control-selected-border': 'overlay.accent',
    'hover': 'sidebar.hover_background', 'window-border': 'sidebar.border',
    'title-bg': 'tab_bar.background', 'title-fg': 'tab_bar.active_foreground',
    'sidebar-bg': 'sidebar.background', 'sidebar-fg': 'sidebar.foreground',
    'sidebar-selected': 'sidebar.selection_background', 'sidebar-selected-fg': 'sidebar.selection_foreground',
    'folder-icon': 'sidebar.folder_icon', 'file-icon': 'sidebar.file_icon',
    'tabs-bg': 'tab_bar.background', 'tab-active-bg': 'tab_bar.active_background',
    'tab-active-fg': 'tab_bar.active_foreground', 'tab-inactive-bg': 'tab_bar.inactive_background',
    'tab-inactive-fg': 'tab_bar.inactive_foreground', 'tab-border': 'tab_bar.border',
    'code-fg': 'editor.foreground', 'current-line': 'editor.current_line_background',
    'gutter-bg': 'gutter.background', 'gutter-fg': 'gutter.foreground',
    'gutter-active': 'gutter.foreground_active',
    'syntax-keyword': 'syntax.keyword', 'syntax-type': 'syntax.type',
    'syntax-fn': 'syntax.function', 'syntax-comment': 'syntax.comment',
    'syntax-string': 'syntax.string', 'syntax-number': 'syntax.number',
    'scrollbar': 'scrollbar.thumb', 'scrollbar-track': 'scrollbar.track',
    'scrollbar-hover': 'scrollbar.thumb_hover', 'splitter': 'splitter.background',
    'input-background': 'overlay.input_background',
    'overlay-panel-bg': 'overlay.panel_background',
    'keycap-fg': 'overlay.keycap_fg', 'keycap-border': 'overlay.keycap_border',
    'severity-error': 'overlay.severity_error', 'severity-warning': 'overlay.severity_warning',
    'severity-info': 'overlay.severity_info',
    'severity-error-text': 'overlay.severity_error_text',
    'severity-warning-text': 'overlay.severity_warning_text',
    'severity-info-text': 'overlay.severity_info_text',
    'bright': 'overlay.text_bright', 'tooltip-bg': 'overlay.background',
    'tooltip-border': 'overlay.border', 'tooltip-fg': 'overlay.foreground',
    'stat-fg': 'overlay.text_primary', 'stage-fg': 'sidebar.foreground',
    'cache-fg': 'overlay.text_primary', 'status-bg': 'status_bar.background',
    'status-fg': 'status_bar.foreground', 'status-border': 'status_bar.border',
    'focus': 'overlay.accent_bright', 'chart-grid': 'overlay.hairline',
    'chart-label': 'overlay.text_dim', 'chart-budget': 'overlay.warning',
    'chart-cursor': 'overlay.text_secondary', 'chart-selected': 'overlay.text_bright',
}


def generate():
    source = (ROOT / 'src/theme.rs').read_text()
    constants = dict(re.findall(r'pub const (\w+): &str = include_str!\("\.\./([^\"]+)"\)', source))
    entries = re.findall(r'BuiltinTheme\s*\{\s*id: "([^\"]+)",\s*yaml: (\w+)', source)
    palettes = []
    for identity, constant in entries:
        relative = constants[constant]
        raw = (ROOT / relative).read_bytes()
        data = yaml.safe_load(raw)
        ui = data['ui']
        ui['overlay'] = resolve_overlay(ui)
        ui['gutter'].setdefault('border_color', '#313438')
        ui['status_bar'].setdefault('border', ui['gutter']['border_color'])
        colors = {}
        for token, field in FIELDS.items():
            group, key = field.split('.')
            value = ui[group][key]
            if not re.fullmatch(r'#[\da-fA-F]{6}(?:[\da-fA-F]{2})?', value):
                raise ValueError(f'{relative}: invalid color {field}={value}')
            colors['--' + token] = value
        series = [ui['syntax'][key] for key in ('type', 'function', 'keyword', 'number', 'attribute', 'string', 'property', 'variable', 'text', 'comment')]
        palettes.append({'id': identity, 'name': data['name'], 'source': relative,
                         'sha256': hashlib.sha256(raw).hexdigest(),
                         'scheme': 'light' if luminance(ui['editor']['background']) > .5 else 'dark',
                         'colors': colors, 'series': series})
    destination = ROOT / 'prototypes/debug-performance-themes.js'
    destination.write_text('// Generated by generate-performance-themes.py; do not hand-edit.\n'
                           '// Source fields and fallback derivations: src/theme.rs.\n'
                           'window.TOKEN_PERFORMANCE_THEMES = ' + json.dumps({'fields': FIELDS, 'themes': palettes}, indent=2) + ';\n')
    print(f'Wrote {len(palettes)} repository themes to {destination.relative_to(ROOT)}')


if __name__ == '__main__':
    generate()
