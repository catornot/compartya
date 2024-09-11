# compartya
compartya is a party system for titanfall 2 northstar

it uses the server browser under the hood so only listed servers would work with this party system

icon made by `skordge`

# Usage (easy) with new northstar
1. in the compartya gui in top left corner start a lobby
2. join a server
3. send a discord invite

# Usage (manually)
1. after creating the lobby
2. copy the lobby id from the console
3. it can be shared and inputed into the gui or via the `p_connect_to_lobby` command

# URI
it's registered when running the game with administrator privileges

then can access with the compartya `uri`
`compartya::\open:{server id or name here}`

# Ip addresses

compartya handles public ips automatically w/ a stun server and local ip is resolved w/ `ipconfig` which can fail in certains

the default port is `12352`

## overwriting
| **command line arg** | **value**    |
| :------------------: | :----------: |
| `compartya_ip`       | ip           |
| `compartya_port`     | port         |

**example:**

```bash
NorthstarLauncher.exe -multiple compartya_ip 127.0.0.1 compartya_port 12352
```
