#!/bin/bash
# x-linear: Linear API CLI module for x-cmd
# Full-featured Linear API integration for agent workflows

# Load configuration
CONFIG_FILE="${HOME}/.config/linear/config.json"
if [ -f "$CONFIG_FILE" ]; then
    API_KEY=$(cat "$CONFIG_FILE" | jq -r '.api_key // empty' 2>/dev/null)
fi
API_KEY="${API_KEY:-}"
# Require LINEAR_API_KEY - no hardcoded fallback
if [ -z "$API_KEY" ] && [ -z "$LINEAR_API_KEY" ]; then
    echo "Error: LINEAR_API_KEY environment variable is required" >&2
    echo "Set it with: export LINEAR_API_KEY=<your-api-key>" >&2
    return 1 2>/dev/null || exit 1
fi
API_KEY="${API_KEY:-${LINEAR_API_KEY}}"
API_URL="https://api.linear.app/graphql"

# Output format variables
OUTPUT_JSON=false
OUTPUT_FORMAT="table"

# Team and state caches
_team_cache=""
_state_cache=""

AGENT_READY_LABEL="agent-ready"
OMC_LEASE_PREFIX="<!-- x-linear-lease "

_json_string_array() {
    if [ "$#" -eq 0 ]; then
        echo "[]"
    else
        printf '%s\n' "$@" | jq -R . | jq -s -c .
    fi
}

# Escape string for GraphQL - prevents injection attacks
_escape_json() {
    local str="$1"
    printf '%s' "$str" | jq -Rs '.' | sed 's/\\n$//'
}

# Validate ID format (accepts UUID or Linear identifier like OMC-6)
_validate_id() {
    local id="$1"
    if [ -z "$id" ]; then
        echo "Error: ID is required" >&2
        return 1
    fi
    # UUID v4 format OR Linear identifier (TEAM-Number, case-insensitive)
    if ! printf '%s' "$id" | grep -qEi '^([0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}|[A-Za-z]+-[0-9]+)$'; then
        echo "Error: Invalid ID format: $id (expected UUID or identifier like OMC-6)" >&2
        return 1
    fi
    return 0
}

# Alias for backwards compatibility
_validate_uuid() { _validate_id "$@"; }

# Validate priority (0-4)
_validate_priority() {
    local priority="$1"
    if [ -z "$priority" ]; then
        return 0  # Priority is optional
    fi
    if ! printf '%s' "$priority" | grep -qE '^[0-4]$'; then
        echo "Error: Priority must be 0-4, got: $priority" >&2
        return 1
    fi
    return 0
}

# Validate positive integer
_validate_positive_int() {
    local val="$1" name="${2:-value}"
    if [ -z "$val" ]; then
        echo "Error: $name is required" >&2
        return 1
    fi
    if ! printf '%s' "$val" | grep -qE '^[1-9][0-9]*$'; then
        echo "Error: $name must be a positive integer, got: $val" >&2
        return 1
    fi
    return 0
}

# GraphQL request helper with error checking
linear_graphql() {
    local response json
    # Convert multiline query to single line, then encode properly
    local single_line=$(echo "$1" | tr '\n' ' ' | sed 's/  */ /g')
    json=$(jq -n --arg q "$single_line" '{"query": $q}')
    response=$(curl -s --connect-timeout 5 -f -H "Authorization: $API_KEY" \
        -H "Content-Type: application/json" \
        "$API_URL" \
        -d "$json") || {
        # Try without -f to get error details
        response=$(curl -s --connect-timeout 5 -H "Authorization: $API_KEY" \
            -H "Content-Type: application/json" \
            "$API_URL" \
            -d "$json")
        if [ -n "$response" ]; then
            echo "$response" | jq -r '.errors[0].message' 2>/dev/null || echo "API error: $response" >&2
        else
            echo "API request failed (curl exit: $?)" >&2
        fi
        return 1
    }

    # Check for GraphQL errors
    if echo "$response" | jq -e '.errors' >/dev/null 2>&1; then
        echo "$response" | jq -r '.errors[0].message' >&2
        return 1
    fi
    echo "$response"
}

# Load team cache
_load_team_cache() {
    if [ -z "$_team_cache" ]; then
        _team_cache=$(linear_graphql "{ teams { nodes { id name key } } }")
    fi
}

# Load state IDs cache
_load_state_cache() {
    if [ -z "$_state_cache" ]; then
        _state_cache=$(linear_graphql "{ teams { nodes { id name states { nodes { id name } } } } }")
    fi
}

# Get team ID by name (accepts name or key, resolves to ID)
_get_team_id() {
    local team_name="$1"
    _load_team_cache
    echo "$_team_cache" | jq -r --arg team "$team_name" \
        '.data.teams.nodes[] | select(.name == $team or .key == $team) | .id'
}

# Get state ID by name for a specific team
_get_state_id_for_team() {
    local team_name="$1" state_name="$2"
    _load_state_cache
    local team_id=$(_get_team_id "$team_name")
    [ -z "$team_id" ] || [ "$team_id" = "null" ] && return 1
    echo "$_state_cache" | jq -r --arg team_id "$team_id" --arg state "$state_name" \
        '.data.teams.nodes[] | select(.id == $team_id) | .states.nodes[] | select(.name == $state) | .id'
}

# Get state ID by name (uses first matching team as fallback)
_get_state_id() {
    local team="${1:-OMC}" state_name="$2"
    _load_state_cache
    echo "$_state_cache" | jq -r --arg team "$team" --arg state "$state_name" \
        '.data.teams.nodes[] | select(.name == $team) | .states.nodes[] | select(.name == $state) | .id'
}

_get_label_id_for_team() {
    local team_name="$1" label_name="$2"
    local team_id=$(_get_team_id "$team_name")
    [ -z "$team_id" ] || [ "$team_id" = "null" ] && return 1
    linear_graphql "{ team(id: \"$team_id\") { labels { nodes { id name } } } }" | \
        jq -r --arg label "$label_name" '.data.team.labels.nodes[] | select(.name == $label or .id == $label) | .id' | head -1
}

_x_linear_active_lease_run_id() {
    local issue_json="$1"
    local now="${2:-$(date +%s)}"
    echo "$issue_json" | jq -r --arg prefix "$OMC_LEASE_PREFIX" --argjson now "$now" '
        [.comments.nodes[]
            | select(.body | contains($prefix))
            | . as $comment
            | (.body | capture("run_id=(?<run_id>[^ ]+) expires_at=(?<expires_at>[0-9]+)")?) as $lease
            | select($lease != null and (($lease.expires_at | tonumber) > $now))
            | {id: $comment.id, createdAt: $comment.createdAt, run_id: $lease.run_id}
        ]
        | sort_by(.createdAt, .id)
        | .[0].run_id // empty
    '
}

# Resolve Linear identifier (OMC-6) to UUID
_resolve_identifier() {
    local identifier="$1"
    # Check if it's already a UUID
    if printf '%s' "$identifier" | grep -qE '^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$'; then
        echo "$identifier"
        return 0
    fi
    # Check if it's a Linear identifier (TEAM-NUMBER)
    if printf '%s' "$identifier" | grep -qE '^[A-Za-z]+-[0-9]+$'; then
        # Extract team key
        local team_key="${identifier%%-*}"
        # Search by team key and filter client-side for the identifier (single line query)
        local result=$(linear_graphql "{ issues(filter: { team: { key: { eq: \"$team_key\" } } }, first: 50) { nodes { id identifier } } }" | jq -r ".data.issues.nodes[] | select(.identifier == \"$identifier\") | .id" 2>/dev/null)
        echo "$result"
    fi
}

# Dynamic state transition - get team from issue first, then find state in that team
_x_linear_set_state() {
    local id="$1" state_name="$2"

    # Resolve identifier to UUID if needed
    id=$(_resolve_identifier "$id")
    [ -z "$id" ] && { echo "Error: Could not resolve issue identifier" >&2; return 1; }

    # Get team from issue first
    local team_id=$(linear_graphql "{ issue(id: \"$id\") { team { id name } } }" | \
        jq -r '.data.issue.team.id // empty')
    local team_name=$(linear_graphql "{ issue(id: \"$id\") { team { id name } } }" | \
        jq -r '.data.issue.team.name // empty')

    [ -z "$team_id" ] || [ "$team_id" = "null" ] && {
        echo "Error: Could not find team for issue $id" >&2
        return 1
    }

    # Find state in that team's states
    local state_id=$(linear_graphql "{ team(id: \"$team_id\") { states { nodes { id name } } } }" | \
        jq -r --arg state "$state_name" '.data.team.states.nodes[] | select(.name == $state) | .id')

    [ -z "$state_id" ] || [ "$state_id" = "null" ] && {
        echo "Error: State '$state_name' not found in team '$team_name'" >&2
        return 1
    }

    # Update the issue
    linear_graphql "mutation { issueUpdate(id: \"$id\", input: { stateId: \"$state_id\" }) { issue { id identifier title state { name } team { name } } } }" | jq '.data.issueUpdate.issue'
}

# Reset output flags to defaults
_reset_output_flags() {
    OUTPUT_JSON=false
    OUTPUT_FORMAT="table"
}

# --- Issue Commands ---

x_linear_issue_list() {
    _reset_output_flags
    local team="" state="" assignee="" priority="" limit=20

    while [[ $# -gt 0 ]]; do
        case $1 in
            --team) team="$2"; shift 2 ;;
            --state) state="$2"; shift 2 ;;
            --assignee) assignee="$2"; shift 2 ;;
            --priority) priority="$2"; shift 2 ;;
            --limit) limit="$2"; shift 2 ;;
            --json) OUTPUT_JSON=true; shift ;;
            --format) OUTPUT_FORMAT="$2"; shift 2 ;;
            *) shift ;;
        esac
    done

    # Validate inputs
    [ -n "$priority" ] && { _validate_priority "$priority" || return 1; }
    [ -n "$limit" ] && { _validate_positive_int "$limit" "limit" || return 1; }

    # Escape user inputs for GraphQL safety
    local esc_team esc_state esc_assignee
    esc_team=$(_escape_json "$team")
    esc_state=$(_escape_json "$state")
    esc_assignee=$(_escape_json "$assignee")

    local filter=""
    [ -n "$team" ] && filter="${filter}team: { name: { eq: $esc_team } }, "
    [ -n "$state" ] && filter="${filter}state: { name: { eq: $esc_state } }, "
    [ -n "$assignee" ] && filter="${filter}assignee: { name: { eq: $esc_assignee } }, "
    [ -n "$priority" ] && filter="${filter}priority: { eq: $priority }, "

    local gql_filter=""
    [ -n "$filter" ] && gql_filter="filter: { ${filter%,} }, "

    local query="{ issues($gql_filter first: $limit, orderBy: updatedAt) { nodes { id identifier title state { name } assignee { name } team { name } createdAt priority dueDate } } }"

    if [ "$OUTPUT_JSON" = true ] || [ "$OUTPUT_FORMAT" = "json" ]; then
        linear_graphql "$query" | jq '.data.issues.nodes[]'
    else
        linear_graphql "$query" | jq -r '.data.issues.nodes[] |
            "\(.identifier) | \(.state.name // "none") | \(.title) | \(.assignee.name // "-") | \(.team.name)"'
    fi
}

x_linear_issue_get() {
    local id="$1"
    [ -z "$id" ] && { echo "Error: issue id required" >&2; return 1; }

    # Resolve identifier to UUID if needed
    id=$(_resolve_identifier "$id")
    [ -z "$id" ] && { echo "Error: Could not resolve issue identifier" >&2; return 1; }

    # Get basic issue data
    local issue=$(linear_graphql "{ issue(id: \"$id\") { id identifier title description state { id name } assignee { id name email } team { id name } createdAt updatedAt dueDate priority } }" | jq '.data.issue')

    if [ "$OUTPUT_JSON" = true ] || [ "$OUTPUT_FORMAT" = "json" ]; then
        echo "$issue"
    else
        # Pretty print for humans
        echo "$issue" | jq '{ id, identifier, title, state: .state.name, assignee: .assignee.name, team: .team.name, createdAt, priority }'
    fi
}

x_linear_issue_create() {
    _reset_output_flags
    local team_name="" title="" description="" priority=""
    local label_ids=() label_names=()

    while [[ $# -gt 0 ]]; do
        case $1 in
            --team) team_name="$2"; shift 2 ;;
            --title) title="$2"; shift 2 ;;
            --description) description="$2"; shift 2 ;;
            --priority) priority="$2"; shift 2 ;;
            --label|--label-name) label_names+=("$2"); shift 2 ;;
            --label-id) label_ids+=("$2"); shift 2 ;;
            --json) OUTPUT_JSON=true; shift ;;
            --format) OUTPUT_FORMAT="$2"; shift 2 ;;
            *) shift ;;
        esac
    done

    [ -z "$team_name" ] || [ -z "$title" ] && { echo "Error: --team (name) and --title required" >&2; return 1; }
    [ -n "$priority" ] && { _validate_priority "$priority" || return 1; }

    # Resolve team name to ID
    local team_id=$(_get_team_id "$team_name")
    [ -z "$team_id" ] || [ "$team_id" = "null" ] && { echo "Error: Team '$team_name' not found" >&2; return 1; }

    local label_name label_id
    for label_name in "${label_names[@]}"; do
        label_id=$(_get_label_id_for_team "$team_name" "$label_name")
        [ -z "$label_id" ] || [ "$label_id" = "null" ] && {
            echo "Error: Label '$label_name' not found in team '$team_name'" >&2
            return 1
        }
        label_ids+=("$label_id")
    done

    # Escape user inputs for GraphQL safety
    local esc_title esc_description
    esc_title=$(_escape_json "$title")
    esc_description=$(_escape_json "$description")

    local input="teamId: \"$team_id\", title: $esc_title"
    [ -n "$description" ] && input="$input, description: $esc_description"
    [ -n "$priority" ] && input="$input, priority: $priority"
    [ ${#label_ids[@]} -gt 0 ] && input="$input, labelIds: $(_json_string_array "${label_ids[@]}")"

    linear_graphql "mutation { issueCreate(input: { $input }) { issue { id identifier title state { name } team { name } } } }" | jq '.data.issueCreate.issue'
}

x_linear_issue_update() {
    local id="$1"; shift
    local title="" description="" state_id="" assignee_id="" priority="" due_date=""

    while [[ $# -gt 0 ]]; do
        case $1 in
            --title) title="$2"; shift 2 ;;
            --description) description="$2"; shift 2 ;;
            --state) state_id="$2"; shift 2 ;;
            --assignee) assignee_id="$2"; shift 2 ;;
            --priority) priority="$2"; shift 2 ;;
            --due) due_date="$2"; shift 2 ;;
            *) shift ;;
        esac
    done

    [ -z "$id" ] && { echo "Error: issue id required" >&2; return 1; }
    _validate_uuid "$id" || return 1
    [ -n "$state_id" ] && { _validate_uuid "$state_id" || return 1; }
    [ -n "$assignee_id" ] && { _validate_uuid "$assignee_id" || return 1; }
    [ -n "$priority" ] && { _validate_priority "$priority" || return 1; }

    # Escape user inputs for GraphQL safety
    local esc_title esc_description esc_due_date
    esc_title=$(_escape_json "$title")
    esc_description=$(_escape_json "$description")
    esc_due_date=$(_escape_json "$due_date")

    local input=""
    [ -n "$title" ] && input="$input title: $esc_title,"
    [ -n "$description" ] && input="$input description: $esc_description,"
    [ -n "$state_id" ] && input="$input stateId: \"$state_id\","
    [ -n "$assignee_id" ] && input="$input assigneeId: \"$assignee_id\","
    [ -n "$priority" ] && input="$input priority: $priority,"
    [ -n "$due_date" ] && input="$input dueDate: $esc_due_date,"

    linear_graphql "mutation { issueUpdate(id: \"$id\", input: { ${input%,} }) { issue { id identifier title state { name } assignee { name } } } }" | jq '.data.issueUpdate.issue'
}

x_linear_issue_delete() {
    local id="$1"
    [ -z "$id" ] && { echo "Error: issue id required" >&2; return 1; }
    _validate_uuid "$id" || return 1
    linear_graphql "mutation { issueDelete(id: \"$id\") { success } }" | jq '.data.issueDelete'
}

x_linear_issue_archive() {
    local id="$1"
    [ -z "$id" ] && { echo "Error: issue id required" >&2; return 1; }
    _validate_uuid "$id" || return 1
    linear_graphql "mutation { issueArchive(id: \"$id\") { issue { id identifier title } } }" | \
        jq '.data.issueArchive.issue'
}

# --- State Transition Commands (Agent Workflow) ---

x_linear_issue_assign() {
    local id="$1" user_id="$2"
    [ -z "$id" ] || [ -z "$user_id" ] && { echo "Error: issue id and user id required" >&2; return 1; }
    _validate_uuid "$id" || return 1
    _validate_uuid "$user_id" || return 1
    linear_graphql "mutation { issueUpdate(id: \"$id\", input: { assigneeId: \"$user_id\" }) { issue { id identifier assignee { name } } } }" | jq '.data.issueUpdate.issue'
}

x_linear_issue_unassign() {
    local id="$1"
    [ -z "$id" ] && { echo "Error: issue id required" >&2; return 1; }
    _validate_uuid "$id" || return 1
    linear_graphql "mutation { issueUpdate(id: \"$id\", input: { assigneeId: null }) { issue { id identifier assignee { name } } } }" | jq '.data.issueUpdate.issue'
}

# Internal state transition functions (used by both shortcut and issue namespaced commands)
_x_linear_done() {
    local id="${1:-}"
    [ -z "$id" ] && { echo "Error: issue id required" >&2; return 1; }
    _validate_uuid "$id" || return 1
    _x_linear_set_state "$id" "Done"
}

_x_linear_start() {
    local id="${1:-}"
    [ -z "$id" ] && { echo "Error: issue id required" >&2; return 1; }
    _validate_uuid "$id" || return 1
    _x_linear_set_state "$id" "In Progress"
}

_x_linear_review() {
    local id="${1:-}"
    [ -z "$id" ] && { echo "Error: issue id required" >&2; return 1; }
    _validate_uuid "$id" || return 1
    _x_linear_set_state "$id" "In Review"
}

_x_linear_cancel() {
    local id="${1:-}"
    [ -z "$id" ] && { echo "Error: issue id required" >&2; return 1; }
    _validate_uuid "$id" || return 1
    _x_linear_set_state "$id" "Canceled"
}

# Shortcut state transition commands
x_linear_done() {
    _reset_output_flags
    _x_linear_done "$@"
}

x_linear_start() {
    _reset_output_flags
    _x_linear_start "$@"
}

x_linear_review() {
    _reset_output_flags
    _x_linear_review "$@"
}

x_linear_cancel() {
    _reset_output_flags
    _x_linear_cancel "$@"
}

# --- Agent Workflow Commands ---

x_linear_claim() {
    _reset_output_flags
    local team=""
    while [[ $# -gt 0 ]]; do
        case $1 in
            --team) team="$2"; shift 2 ;;
            --json) OUTPUT_JSON=true; shift ;;
            --format) OUTPUT_FORMAT="$2"; shift 2 ;;
            *) shift ;;
        esac
    done
    team="${team:-OMC-RS}"

    local user_id=$(linear_graphql "{ viewer { id } }" | jq -r '.data.viewer.id')
    local progress_id=$(_get_state_id_for_team "$team" "In Progress")

    # Escape team name
    local esc_team=$(_escape_json "$team")

    # Find first agent-ready issue and exclude active leases/progress states client-side.
    local now=$(date +%s)
    local after_arg="null" issue_data unassigned="" has_next cursor
    while :; do
        issue_data=$(linear_graphql "{ issues(filter: { team: { name: { eq: $esc_team } } }, first: 50, after: $after_arg, orderBy: createdAt) { pageInfo { hasNextPage endCursor } nodes { id identifier title state { name } assignee { id name } labels { nodes { name } } comments(first: 100) { nodes { id body createdAt } } } } }")
        unassigned=$(echo "$issue_data" | jq -r --arg ready "$AGENT_READY_LABEL" --arg prefix "$OMC_LEASE_PREFIX" --argjson now "$now" '
            .data.issues.nodes[]
            | select(.assignee == null)
            | select([.labels.nodes[].name] | index($ready))
            | select((.state.name | ascii_downcase) as $state | ["in progress", "in review", "done", "canceled", "cancelled"] | index($state) | not)
            | select(([.comments.nodes[]
                | select(.body | contains($prefix))
                | (.body | capture("expires_at=(?<expires_at>[0-9]+)")?) as $lease
                | select($lease != null and (($lease.expires_at | tonumber) > $now))
            ] | length) == 0)
            | "\(.id)|\(.title)"
        ' | head -1)
        [ -n "$unassigned" ] && break
        has_next=$(echo "$issue_data" | jq -r '.data.issues.pageInfo.hasNextPage')
        [ "$has_next" != "true" ] && break
        cursor=$(echo "$issue_data" | jq -r '.data.issues.pageInfo.endCursor // empty')
        [ -z "$cursor" ] && break
        after_arg=$(_escape_json "$cursor")
    done
    local issue_id="${unassigned%%|*}"
    local issue_title="${unassigned#*|}"

    if [ -z "$issue_id" ] || [ "$issue_id" = "null" ] || [ -z "$issue_title" ] || [ "$issue_title" = "null" ]; then
        echo "{\"error\": \"No unassigned agent-ready issues found in team $team\"}" >&2
        return 1
    fi

    local run_id="x-linear-$(date +%s)-$$-$RANDOM"
    local expires_at=$(( $(date +%s) + 7200 ))
    local lease_body="${OMC_LEASE_PREFIX}run_id=${run_id} expires_at=${expires_at} -->"$'\n'"x-linear lease for \`${run_id}\`."
    local esc_lease_body=$(_escape_json "$lease_body")
    linear_graphql "mutation { commentCreate(input: { issueId: \"$issue_id\", body: $esc_lease_body }) { comment { id } } }" >/dev/null || return 1

    local issue_after_lease=$(linear_graphql "{ issue(id: \"$issue_id\") { id identifier title state { name } assignee { id name } labels { nodes { name } } comments(first: 100) { nodes { id body createdAt } } } }")
    local winner=$(_x_linear_active_lease_run_id "$(echo "$issue_after_lease" | jq '.data.issue')" "$(date +%s)")
    if [ "$winner" != "$run_id" ]; then
        echo "{\"error\": \"Lost claim race for issue $issue_id\", \"winner\": \"$winner\"}" >&2
        return 1
    fi

    # Assign and move to In Progress
    local update_result
    if [ -n "$progress_id" ]; then
        update_result=$(linear_graphql "mutation { issueUpdate(id: \"$issue_id\", input: { assigneeId: \"$user_id\", stateId: \"$progress_id\" }) { issue { id identifier title state { name } assignee { id name } } } }")
    else
        update_result=$(linear_graphql "mutation { issueUpdate(id: \"$issue_id\", input: { assigneeId: \"$user_id\" }) { issue { id identifier title state { name } assignee { id name } } } }")
    fi
    local assigned_to=$(echo "$update_result" | jq -r '.data.issueUpdate.issue.assignee.id // empty')
    if [ "$assigned_to" != "$user_id" ]; then
        echo "{\"error\": \"Claim verification failed\", \"issue\": \"$issue_id\"}" >&2
        return 1
    fi
    echo "$update_result" | jq '.data.issueUpdate.issue'
}

x_linear_my() {
    _reset_output_flags
    while [[ $# -gt 0 ]]; do
        case $1 in
            --json) OUTPUT_JSON=true; shift ;;
            --format) OUTPUT_FORMAT="$2"; shift 2 ;;
            *) shift ;;
        esac
    done

    local user_id=$(linear_graphql "{ viewer { id } }" | jq -r '.data.viewer.id')
    local data=$(linear_graphql "{ issues(filter: { assignee: { id: { eq: \"$user_id\" } } }, first: 20, orderBy: updatedAt) { nodes { id identifier title state { name } team { name } priority dueDate } } }")

    if [ "$OUTPUT_JSON" = true ] || [ "$OUTPUT_FORMAT" = "json" ]; then
        echo "$data" | jq '.data.issues.nodes[]'
    else
        echo "$data" | jq -r '.data.issues.nodes[] | "\(.identifier) | \(.state.name) | \(.priority // 0) | \(.title) | \(.team.name) | due:\(.dueDate // "-")"'
    fi
}

x_linear_next() {
    _reset_output_flags
    local team=""
    while [[ $# -gt 0 ]]; do
        case $1 in
            --team) team="$2"; shift 2 ;;
            --json) OUTPUT_JSON=true; shift ;;
            --format) OUTPUT_FORMAT="$2"; shift 2 ;;
            *) shift ;;
        esac
    done
    team="${team:-OMC-RS}"

    local esc_team=$(_escape_json "$team")
    local now=$(date +%s)
    local after_arg="null" ready_issues='[]' issue_data page_ready has_next cursor count
    while :; do
        issue_data=$(linear_graphql "{ issues(filter: { team: { name: { eq: $esc_team } } }, first: 50, after: $after_arg, orderBy: createdAt) { pageInfo { hasNextPage endCursor } nodes { id identifier title state { name } priority assignee { name } labels { nodes { name } } comments(first: 100) { nodes { body } } } } }")
        page_ready=$(echo "$issue_data" | jq --arg ready "$AGENT_READY_LABEL" --arg prefix "$OMC_LEASE_PREFIX" --argjson now "$now" '
            [.data.issues.nodes[]
                | select(.assignee == null)
                | select([.labels.nodes[].name] | index($ready))
                | select((.state.name | ascii_downcase) as $state | ["in progress", "in review", "done", "canceled", "cancelled"] | index($state) | not)
                | select(([.comments.nodes[]
                    | select(.body | contains($prefix))
                    | (.body | capture("expires_at=(?<expires_at>[0-9]+)")?) as $lease
                    | select($lease != null and (($lease.expires_at | tonumber) > $now))
                ] | length) == 0)
                | {id, identifier, title, state, priority, assignee}
            ]')
        ready_issues=$(jq -n --argjson old "$ready_issues" --argjson page "$page_ready" '$old + $page | .[:20]')
        count=$(echo "$ready_issues" | jq 'length')
        [ "$count" -ge 20 ] && break
        has_next=$(echo "$issue_data" | jq -r '.data.issues.pageInfo.hasNextPage')
        [ "$has_next" != "true" ] && break
        cursor=$(echo "$issue_data" | jq -r '.data.issues.pageInfo.endCursor // empty')
        [ -z "$cursor" ] && break
        after_arg=$(_escape_json "$cursor")
    done

    if [ "$OUTPUT_JSON" = true ] || [ "$OUTPUT_FORMAT" = "json" ]; then
        echo "$ready_issues"
    else
        echo "$ready_issues" | jq -r '.[] |
            "\(.identifier) | P\(.priority // 0) | \(.state.name) | \(.title)"'
    fi
}

# --- Comment Commands ---

x_linear_comment_list() {
    local id="$1"
    [ -z "$id" ] && { echo "Error: issue id required" >&2; return 1; }
    id=$(_resolve_identifier "$id")
    [ -z "$id" ] && { echo "Error: Could not resolve issue identifier" >&2; return 1; }
    linear_graphql "{ issue(id: \"$id\") { comments { nodes { id body createdAt } } } }" | jq '.data.issue.comments.nodes[]'
}

x_linear_comment_add() {
    local issue_id="" body=""
    while [[ $# -gt 0 ]]; do
        case $1 in
            --issue) issue_id="$2"; shift 2 ;;
            --body) body="$2"; shift 2 ;;
            *) issue_id="$1"; shift ;;
        esac
    done
    [ -z "$issue_id" ] || [ -z "$body" ] && { echo "Error: issue id and --body required" >&2; return 1; }

    # Resolve identifier to UUID if needed
    issue_id=$(_resolve_identifier "$issue_id")
    [ -z "$issue_id" ] && { echo "Error: Could not resolve issue identifier" >&2; return 1; }

    # Escape body content
    local esc_body=$(_escape_json "$body")
    linear_graphql "mutation { commentCreate(input: { issueId: \"$issue_id\", body: $esc_body }) { comment { id body createdAt } } }" | jq '.data.commentCreate.comment'
}

x_linear_comment_update() {
    local id="" body=""
    while [[ $# -gt 0 ]]; do
        case $1 in
            --body) body="$2"; shift 2 ;;
            *) if [ -z "$id" ]; then id="$1"; else body="$1"; fi; shift ;;
        esac
    done
    [ -z "$id" ] || [ -z "$body" ] && { echo "Error: comment id and --body required" >&2; return 1; }
    _validate_uuid "$id" || return 1

    # Escape body content
    local esc_body=$(_escape_json "$body")
    linear_graphql "mutation { commentUpdate(id: \"$id\", input: { body: $esc_body }) {
        comment { id body createdAt }
    } }" | jq '.data.commentUpdate.comment'
}

# --- Team Commands ---

x_linear_team_list() {
    _reset_output_flags
    while [[ $# -gt 0 ]]; do
        case $1 in
            --json) OUTPUT_JSON=true; shift ;;
            --format) OUTPUT_FORMAT="$2"; shift 2 ;;
            *) shift ;;
        esac
    done

    local data=$(linear_graphql "{ teams { nodes { id name key } } }")
    if [ "$OUTPUT_JSON" = true ] || [ "$OUTPUT_FORMAT" = "json" ]; then
        echo "$data" | jq '.data.teams.nodes[]'
    else
        echo "$data" | jq -r '.data.teams.nodes[] | "\(.key) | \(.name) | \(.id)"'
    fi
}

x_linear_team_get() {
    _reset_output_flags
    local identifier="$1"
    [ -z "$identifier" ] && { echo "Error: team name or id required"; return 1; }

    while [[ $# -gt 0 ]]; do
        case $1 in
            --json) OUTPUT_JSON=true; shift ;;
            --format) OUTPUT_FORMAT="$2"; shift 2 ;;
            *) shift ;;
        esac
    done

    # Try as name first, then as ID
    local team_id=$(_get_team_id "$identifier")
    if [ -z "$team_id" ] || [ "$team_id" = "null" ]; then
        team_id="$identifier"
    fi

    linear_graphql "{ team(id: \"$team_id\") { id name key states { nodes { id name type color } } } }" | \
        jq '.data.team'
}

x_linear_team_create() {
    local name="" key=""
    while [[ $# -gt 0 ]]; do
        case $1 in
            --name) name="$2"; shift 2 ;;
            --key) key="$2"; shift 2 ;;
            *) shift ;;
        esac
    done
    [ -z "$name" ] || [ -z "$key" ] && { echo "Error: --name and --key required" >&2; return 1; }

    # Escape inputs
    local esc_name=$(_escape_json "$name")
    local esc_key=$(_escape_json "$key")
    linear_graphql "mutation { teamCreate(input: { name: $esc_name, key: $esc_key }) { team { id name key } } }" | \
        jq '.data.teamCreate.team'
}

x_linear_team_members() {
    _reset_output_flags
    local identifier="$1"
    [ -z "$identifier" ] && { echo "Error: team name or id required"; return 1; }

    while [[ $# -gt 0 ]]; do
        case $1 in
            --json) OUTPUT_JSON=true; shift ;;
            --format) OUTPUT_FORMAT="$2"; shift 2 ;;
            *) shift ;;
        esac
    done

    local team_id=$(_get_team_id "$identifier")
    if [ -z "$team_id" ] || [ "$team_id" = "null" ]; then
        team_id="$identifier"
    fi

    linear_graphql "{ team(id: \"$team_id\") { members { nodes { id name email } } } }" | \
        jq '.data.team.members.nodes[]'
}

x_linear_team_states() {
    _reset_output_flags
    local identifier="$1"
    [ -z "$identifier" ] && { echo "Error: team name or id required"; return 1; }

    while [[ $# -gt 0 ]]; do
        case $1 in
            --json) OUTPUT_JSON=true; shift ;;
            --format) OUTPUT_FORMAT="$2"; shift 2 ;;
            *) shift ;;
        esac
    done

    local team_id=$(_get_team_id "$identifier")
    if [ -z "$team_id" ] || [ "$team_id" = "null" ]; then
        team_id="$identifier"
    fi

    linear_graphql "{ team(id: \"$team_id\") { states { nodes { id name type color } } } }" | \
        jq '.data.team.states.nodes[]'
}

# --- User Commands ---

x_linear_user_list() {
    local limit=20
    [ "$1" = "--limit" ] && limit="$2"
    [ -n "$limit" ] && { _validate_positive_int "$limit" "limit" || return 1; }
    linear_graphql "{ users(first: $limit) { nodes { id name email } } }" | \
        jq -r '.data.users.nodes[] | "\(.name) | \(.email)"'
}

x_linear_user_me() {
    _reset_output_flags
    while [[ $# -gt 0 ]]; do
        case $1 in
            --json) OUTPUT_JSON=true; shift ;;
            --format) OUTPUT_FORMAT="$2"; shift 2 ;;
            *) shift ;;
        esac
    done

    linear_graphql "{ viewer { id name email admin createdAt } }" | jq '.data.viewer'
}

x_linear_user_get() {
    local id="$1"
    [ -z "$id" ] && { echo "Error: user id required" >&2; return 1; }
    _validate_uuid "$id" || return 1
    linear_graphql "{ user(id: \"$id\") { id name email } }" | jq '.data.user'
}

# --- Label Commands ---

x_linear_label_list() {
    _reset_output_flags
    local identifier=""
    while [[ $# -gt 0 ]]; do
        case $1 in
            --team) identifier="$2"; shift 2 ;;
            --json) OUTPUT_JSON=true; shift ;;
            --format) OUTPUT_FORMAT="$2"; shift 2 ;;
            *) identifier="$1"; shift ;;
        esac
    done

    local data=""
    if [ -n "$identifier" ]; then
        local team_id=$(_get_team_id "$identifier")
        if [ -z "$team_id" ] || [ "$team_id" = "null" ]; then
            team_id="$identifier"
        fi
        data=$(linear_graphql "{ team(id: \"$team_id\") { labels { nodes { id name color } } } }")
        data=$(echo "$data" | jq '.data.team.labels.nodes[]')
    else
        data=$(linear_graphql "{ issueLabels(first: 50) { nodes { id name color team { name } } } }")
        data=$(echo "$data" | jq '.data.issueLabels.nodes[]')
    fi

    if [ "$OUTPUT_JSON" = true ] || [ "$OUTPUT_FORMAT" = "json" ]; then
        echo "$data"
    else
        echo "$data" | jq -r '"\(.name) | \(.color) | \(.team.name // "-")"'
    fi
}

# --- Search ---

x_linear_search() {
    local query="$1" type="issue" team=""
    [ -n "$query" ] && shift
    if [[ $# -gt 0 && "$1" != --* ]]; then
        type="$1"
        shift
    fi
    while [[ $# -gt 0 ]]; do
        case $1 in
            --team) team="$2"; shift 2 ;;
            --type) type="$2"; shift 2 ;;
            *) shift ;;
        esac
    done

    [ -z "$query" ] && { echo "Error: search query required" >&2; return 1; }

    case $type in
        issue)
            # Escape user inputs
            local esc_query=$(_escape_json "$query")
            local esc_team=$(_escape_json "$team")
            local filter="or: [{ title: { contains: $esc_query } }, { description: { contains: $esc_query } }]"
            [ -n "$team" ] && filter="team: { name: { eq: $esc_team } }, $filter"
            linear_graphql "{ issues(filter: { $filter }, first: 20) { nodes { id identifier title state { name } assignee { name } team { name } } } }" | jq '.data.issues.nodes[]'
            ;;
        *)
            echo "Error: unknown search type: $type" >&2
            return 1
            ;;
    esac
}

# --- Statistics ---

x_linear_stats() {
    _reset_output_flags
    local team=""

    while [[ $# -gt 0 ]]; do
        case $1 in
            --team) team="$2"; shift 2 ;;
            --json) OUTPUT_JSON=true; shift ;;
            --format) OUTPUT_FORMAT="$2"; shift 2 ;;
            *) shift ;;
        esac
    done
    team="${team:-OMC-RS}"

    local esc_team=$(_escape_json "$team")
    local after_arg="null" states='[]' data has_next cursor
    while :; do
        data=$(linear_graphql "{ issues(filter: { team: { name: { eq: $esc_team } } }, first: 100, after: $after_arg) { pageInfo { hasNextPage endCursor } nodes { state { name } } } }")
        states=$(jq -n --argjson old "$states" --argjson page "$data" '$old + ($page.data.issues.nodes | map(.state.name))')
        has_next=$(echo "$data" | jq -r '.data.issues.pageInfo.hasNextPage')
        [ "$has_next" != "true" ] && break
        cursor=$(echo "$data" | jq -r '.data.issues.pageInfo.endCursor // empty')
        [ -z "$cursor" ] && break
        after_arg=$(_escape_json "$cursor")
    done

    if [ "$OUTPUT_JSON" = true ] || [ "$OUTPUT_FORMAT" = "json" ]; then
        echo "$states" | jq --arg team "$team" '{team: $team, states: group_by(.) | map({state: .[0], count: length})}'
    else
        echo "Team: $team"
        echo "$states" | jq -r '.[]' | sort | uniq -c | while read count state; do
            echo "  $state: $count"
        done
    fi
}

# --- Main ---

x_linear() {
    local cmd="${1:-}"; shift

    case $cmd in
        # Issue
        issue|issues)
            local action="${1:-list}"; shift
            case $action in
                list) x_linear_issue_list "$@" ;;
                get) x_linear_issue_get "$@" ;;
                create) x_linear_issue_create "$@" ;;
                update) x_linear_issue_update "$@" ;;
                delete) x_linear_issue_delete "$@" ;;
                archive) x_linear_issue_archive "$@" ;;
                assign) x_linear_issue_assign "$@" ;;
                unassign) x_linear_issue_unassign "$@" ;;
                # State transitions under issue namespace
                start) _x_linear_start "$@" ;;
                done|close) _x_linear_done "$@" ;;
                review) _x_linear_review "$@" ;;
                cancel) _x_linear_cancel "$@" ;;
                *) echo "Unknown issue action: $action" >&2 ;;
            esac
            ;;
        # State transitions (shortcut commands)
        start) x_linear_start "$@" ;;
        done|close) x_linear_done "$@" ;;
        review) x_linear_review "$@" ;;
        cancel) x_linear_cancel "$@" ;;
        # Agent workflow
        claim) x_linear_claim "$@" ;;
        my|mine) x_linear_my "$@" ;;
        next) x_linear_next "$@" ;;
        # Team
        team|teams)
            local action="${1:-list}"; shift
            case $action in
                list) x_linear_team_list ;;
                get) x_linear_team_get "$@" ;;
                create) x_linear_team_create "$@" ;;
                members) x_linear_team_members "$@" ;;
                states) x_linear_team_states "$@" ;;
                *) echo "Unknown team action: $action" >&2 ;;
            esac
            ;;
        # User
        user|users)
            local action="${1:-me}"; shift
            case $action in
                list) x_linear_user_list "$@" ;;
                me) x_linear_user_me ;;
                get) x_linear_user_get "$@" ;;
                *) echo "Unknown user action: $action" >&2 ;;
            esac
            ;;
        # Comment
        comment|comments)
            local action="${1:-list}"; shift
            case $action in
                list) x_linear_comment_list "$@" ;;
                add) x_linear_comment_add "$@" ;;
                create) x_linear_comment_add "$@" ;;
                update) x_linear_comment_update "$@" ;;
                *) echo "Unknown comment action: $action" >&2 ;;
            esac
            ;;
        # Label
        label|labels)
            local action="${1:-list}"; shift
            case $action in
                list) x_linear_label_list "$@" ;;
                *) echo "Unknown label action: $action" >&2 ;;
            esac
            ;;
        # Search
        search) x_linear_search "$@" ;;
        # Stats
        stats) x_linear_stats "$@" ;;
        # Help
        help|--help|-h) x_linear_help ;;
        *) x_linear help ;;
    esac
}

x_linear_help() {
    cat << 'EOF'
x-linear - Linear API CLI for x-cmd

Usage: x linear <command> [options]

=== Issue Commands ===
  issue list [--team <name>] [--state <name>] [--assignee <name>] [--priority N] [--limit N] [--json] [--format table|json]
  issue get <id> [--json] [--format table|json]
  issue create --team <name> --title <title> [--description <desc>] [--priority N] [--label <name>] [--json] [--format table|json]
  issue update <id> [--title <t>] [--state <id>] [--assignee <id>] [--priority N] [--due <date>] [--json] [--format table|json]
  issue assign <id> <user-id>
  issue unassign <id>
  issue delete <id>
  issue archive <id>

=== State Transitions ===
  # Full commands (under issue namespace)
  issue start <issue-id>   - Move to In Progress
  issue done <issue-id>    - Move to Done
  issue review <issue-id> - Move to In Review
  issue cancel <issue-id> - Move to Canceled

  # Shortcut commands (convenience)
  start <issue-id>         - Move to In Progress
  done <issue-id>          - Move to Done
  review <issue-id>        - Move to In Review
  cancel <issue-id>        - Move to Canceled

  Note: State transitions dynamically look up the issue's team and find the matching state.

=== Agent Workflow ===
  claim [--team <name>]   - Claim next unassigned issue from team
  my [--json] [--format]   - List issues assigned to me
  next [--team <name>]    - Show next agent-ready available issues

=== Team Commands ===
  team list [--json] [--format table|json]
  team get <name-or-id>    - Get team details (accepts team name or ID)
  team create --name <name> --key <key>
  team members <name-or-id> - List team members
  team states <name-or-id>  - List team workflow states

=== User Commands ===
  user list [--limit N] [--json] [--format table|json]
  user me [--json] [--format table|json]
  user get <id>

=== Comment Commands ===
  comment list <issue-id>
  comment add [--issue <id>] --body <text> [--json] [--format table|json]
  comment update <id> --body <text>

=== Label Commands ===
  label list [--team <name>]   - List all labels (optionally filtered by team) [--json] [--format table|json]

=== Search & Stats ===
  search <query> [--team <name>] [--json] [--format table|json]
  stats [--team <name>]        - Show issue counts by state [--json] [--format table|json]

=== Output Formats ===
  --json    - Output as JSON
  --format  - Output format: table (default) or json

Examples:
  x linear team list
  x linear issue create --team OMC --title "Fix bug"
  x linear issue done OMC-123           # Dynamic team lookup
  x linear done OMC-123                  # Shortcut form
  x linear my --json
  x linear label list --team OMC
  x linear stats --team OMC --json
  x linear team states OMC
EOF
}

# Export for x-cmd integration
export -f linear_graphql _load_team_cache _load_state_cache
export -f _get_team_id _get_state_id _get_state_id_for_team _get_label_id_for_team
export -f _x_linear_set_state _reset_output_flags _escape_json
export -f _json_string_array _x_linear_active_lease_run_id
export -f _validate_uuid _validate_priority _validate_positive_int
export -f x_linear
export -f x_linear_issue_list x_linear_issue_get x_linear_issue_create x_linear_issue_update
export -f x_linear_issue_delete x_linear_issue_archive x_linear_issue_assign x_linear_issue_unassign
export -f x_linear_done x_linear_start x_linear_review x_linear_cancel
export -f x_linear_claim x_linear_my x_linear_next
export -f x_linear_comment_list x_linear_comment_add x_linear_comment_update
export -f x_linear_team_list x_linear_team_get x_linear_team_create x_linear_team_members x_linear_team_states
export -f x_linear_user_list x_linear_user_me x_linear_user_get
export -f x_linear_label_list x_linear_search x_linear_stats x_linear_help

[[ "${BASH_SOURCE[0]}" == "${0}" ]] && x_linear "$@"
