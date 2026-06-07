{{- define "password-vault.name" -}}
{{- default .Chart.Name .Values.nameOverride | trunc 63 | trimSuffix "-" -}}
{{- end -}}

{{- define "password-vault.fullname" -}}
{{- printf "%s" (include "password-vault.name" .) | trunc 63 | trimSuffix "-" -}}
{{- end -}}

{{- define "password-vault.namespace" -}}
{{- if .Values.namespaceOverride -}}
{{ .Values.namespaceOverride }}
{{- else -}}
{{ .Release.Namespace }}
{{- end -}}
{{- end -}}

{{- define "password-vault.labels" -}}
app.kubernetes.io/name: {{ include "password-vault.name" . }}
app.kubernetes.io/instance: {{ .Release.Name }}
app.kubernetes.io/version: {{ .Chart.AppVersion | quote }}
app.kubernetes.io/managed-by: {{ .Release.Service }}
app.kubernetes.io/part-of: password-vault
{{- end -}}

{{- define "password-vault.selectorLabels" -}}
app.kubernetes.io/name: {{ include "password-vault.name" . }}
app.kubernetes.io/instance: {{ .Release.Name }}
component: api
{{- end -}}

{{- define "password-vault.image" -}}
{{- if .Values.image.digest -}}
{{ .Values.image.repository }}@{{ .Values.image.digest }}
{{- else -}}
{{ .Values.image.repository }}:{{ default .Chart.AppVersion .Values.image.tag }}
{{- end -}}
{{- end -}}
