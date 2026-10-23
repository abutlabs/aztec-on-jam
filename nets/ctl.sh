# Sourced by nets/net.sh (Docker nets) and nets/jb.sh (native nets): the demo's three chain
# calls over `rpc`, which each defines as `jamboree rpc` against its net.
#   ctl head                      best and finalized slot and hash
#   ctl read <sid> <key>          a storage value at the best block: hex, or (none)
#   ctl submit <sid> <payload>    one work-item in its own package; waits (WAIT seconds) and
#                                 prints "submitted <receipt>" and each "status <JIP-2 status>"
ctl() {
  case "${1:-}" in
    head) rpc head ;;
    read) local k=$3; [[ "$k" =~ ^[0-9a-fA-F]+$ && ${#k} -gt 8 && $(( ${#k} % 2 )) -eq 0 ]] && k=0x$k; rpc read "$2" "$k" ;;
    submit) local out; out=$(rpc submit "$2" "0x$3" --wait --timeout "${WAIT:-240}" 2>&1) || true
      echo "$out" | head -1 | sed 's/^/submitted /'
      echo "$out" | sed -n 's/^{"status":\(.*\)}$/status \1/p' ;;
    *) echo "ctl: head | read <sid> <key> | submit <sid> <payload-hex>" >&2; return 2 ;;
  esac
}
