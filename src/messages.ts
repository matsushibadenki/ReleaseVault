import type {T} from './types'
export function errorText(code:string,t:T){switch(code){
case 'auth':return t('パスワードが違うか、データが破損しています。','Incorrect password or damaged vault.','密码错误或保险库已损坏。')
case 'weak':return t('12文字以上のパスワードを入力してください。','Use at least 12 characters.','请输入至少 12 个字符。')
case 'occupied':return t('既存の値は上書きしません。対象項目を確認してください。','Existing values will not be overwritten. Check the selected items.','不会覆盖已有值。请检查所选项目。')
case 'gh':return t('GitHub CLIのインストール、gh auth login、登録先の権限を確認してください。','Check that GitHub CLI is installed, signed in, and authorized for this destination.','请确认 GitHub CLI 已安装、已登录且有目标仓库权限。')
case 'limit':return t('GitHubの送信上限は48 KBです。','GitHub values must fit within 48 KB.','GitHub 值大小不能超过 48 KB。')
case 'csv':return t('CSVの列は key,name,kind,value,expires の順で指定します。','Expected CSV header: key,name,kind,value,expires.','CSV 列顺序应为 key,name,kind,value,expires。')
case 'mismatch':return t('公開鍵・署名またはエンドポイントが一致しません。','Public key, signature, or endpoint did not match.','公钥、签名或端点不匹配。')
case 'destination':return t('登録先の形式を確認してください。例: owner/repo','Check the destination. Example: owner/repo','请检查目标格式，例如 owner/repo。')
case 'duplicate':return t('このキー名は既に登録されています。','This key name already exists.','此密钥名称已存在。')
case 'locked':return t('保管庫はロックされています。','The vault is locked.','保险库已锁定。')
case 'missing':return t('必要な値が未登録です。','A required value is missing.','缺少必需的值。')
case 'busy':return t('処理中です。','Operation in progress.','正在处理。')
case 'scope':case 'conflict':return t('他の画面で内容が変わりました。再選択してください。','The workspace changed. Select the item again.','工作区已更改。请重新选择项目。')
case 'size':return t('値が空か、サイズの上限を超えています。','The value is empty or too large.','值为空或超出大小限制。')
case 'format':return t('ファイル形式を確認してください。','Check the file format.','请检查文件格式。')
default:return t('操作を完了できませんでした。入力と保存先を確認してください。','Could not complete the operation. Check the input and destination.','操作未完成。请检查输入和保存位置。')+' ('+code.slice(0,48)+')'
}}
