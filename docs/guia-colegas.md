# OuveAI: guia rápido

O OuveAI transcreve as suas reuniões **no seu computador**. Nada do que é falado sai da máquina: não há nuvem, conta nem assinatura.

## 1. Instalar (2 min)

1. Baixe `OuveAI_<versão>_x64-setup.exe` no link que você recebeu.
2. *(Opcional)* Confira o arquivo: no PowerShell, rode `Get-FileHash .\OuveAI_*_x64-setup.exe`. O resultado deve ser igual ao SHA-256 publicado junto do instalador.
3. Dê dois cliques. O instalador não pede administrador.
   - Se aparecer **"O Windows protegeu o computador"**, clique em **Mais informações › Executar assim mesmo**. O aviso aparece porque o instalador ainda não tem assinatura digital.
   - Se o **antivírus** (Kaspersky, por exemplo) perguntar se o OuveAI pode usar o microfone ou o áudio, **permita**. Ele volta a perguntar a cada versão nova.

Requisitos: Windows 10 versão 2004 ou mais novo, 64 bits, 8 GB de RAM e cerca de 1 GB livre.

## 2. Primeiro uso: o modelo de voz (uma vez só)

O assistente do primeiro uso baixa o modelo de reconhecimento (~640 MB). Ele confirma antes de baixar.
- **Rede da empresa bloqueou o download?** Use **Baixar pelo navegador**. Quando terminar, clique em **Importar arquivo…** e escolha o arquivo baixado.
- **Alguém já tem o modelo?** Ele pode passar o arquivo `.tar.bz2` por um pendrive ou pasta compartilhada. Depois é só usar **Importar arquivo…**.

## 3. Primeira reunião

1. Entre na reunião (Teams, Meet, Zoom… qualquer app).
2. Aperte **`Ctrl+Alt+Shift+R`** ou clique em **Gravar reunião**. A janelinha (a "mini") aparece no canto.
3. O texto chega em 2 a 4 segundos:
   - **EU** é o seu microfone;
   - **OUTROS** é o som da reunião.
4. Ao terminar, aperte **`Ctrl+Alt+Shift+R`** de novo. A reunião fica salva em **Sessões**.

| Atalho | O que faz |
|---|---|
| `Ctrl+Alt+Shift+R` | Gravar / parar |
| `Ctrl+Alt+Shift+M` | Marcar um momento importante (⭐) |
| `Ctrl+Alt+Shift+O` | Mostrar / ocultar a mini |

Dá para trocar os atalhos em **Configurações**. Funcionam mesmo com o OuveAI minimizado.

**A mini não aparece quando você compartilha a tela**, então os outros não a veem. Dá para mudar isso no ícone de olho da própria mini.

## 4. Fone ou alto-falante

- **De fone:** é o melhor cenário.
- **No alto-falante:** o microfone também capta a voz dos outros. O OuveAI percebe esse eco e o esconde da transcrição. Se aparecer fala dos outros como EU, avise.
- **"OUTROS sem som há 1 min"?** A reunião está tocando num dispositivo diferente do que o OuveAI escuta. Clique em **Escolher saída**.

## 5. Depois da reunião

Em **Sessões** você pode:
- **Buscar** palavras em todas as reuniões. Não precisa de acento.
- **Copiar transcrição** e colar no chat do Teams ou num e-mail. O texto já vai formatado.
- **Exportar** em `.md`, `.txt`, `.srt` ou `.json`.
- **Ouvir um trecho:** clique no horário da fala. Para isso, ative **Configurações › Áudio › Salvar o áudio da reunião**; o padrão é não salvar.

O OuveAI **não faz resumo nem ata**. Ele entrega a transcrição para você usar onde quiser.

## 6. Privacidade

- Tudo fica em `%LOCALAPPDATA%\br.com.ouveai`, só no seu computador.
- A internet é usada só para:
  - baixar o modelo e o FFmpeg, sempre com a sua confirmação;
  - **Verificar atualização**, quando você clica.
- O áudio só é salvo se você escolher, e é apagado sozinho ao vencer (7 ou 30 dias).
- **Configurações › Dados neste computador › Excluir tudo** apaga todas as reuniões e áudios.
- Avise os participantes de que você está transcrevendo, como faria ao gravar.

## 7. Deu problema?

1. Vá em **Configurações › Sobre › Exportar diagnóstico…** e mande o `.zip` para quem está ajudando. Ele **não** tem nenhuma transcrição, áudio nem título de reunião.
2. Conte o que aconteceu e em qual app de reunião.

**Atualizar:** use **Configurações › Sobre › Verificar atualização**. Se houver versão nova, o OuveAI abre a página de download. Instale por cima, e as reuniões continuam lá.

**Desinstalar:** use Configurações do Windows › Aplicativos › OuveAI. A caixa **"Apagar também reuniões, áudios e o modelo baixado"** vem desmarcada; marque-a se quiser limpar tudo.
