export const meta = {
  name: 'enxame-fluxo',
  description: 'Onda 2: executa, grupo a grupo, as micro-tasks de UM fluxo (operários paralelos que leem o brief do graph.json, verificação por grupo e até 2 rodadas de correção)',
  phases: [
    { title: 'Operários', detail: 'tasks do grupo em paralelo; as "final" depois, uma por vez' },
    { title: 'Verificação', detail: 'supervisor roda o comando do grupo' },
    { title: 'Correção', detail: 'operários corrigem as falhas (rodada 2 sobe para opus)' },
  ],
}

// args = { stream, graph_path, repo, groups: [{ id, verificacao_grupo, tasks: [{ id, tier, final, arquivos }] }], only?: [groupIds] }
// Os BRIEFS ficam no graph.json (os operários o leem); aqui só passam metadados.

const STR = { type: 'string' }
const LISTA = { type: 'array', items: STR }

const RELATORIO = {
  type: 'object',
  required: ['task_id', 'status', 'arquivos'],
  properties: {
    task_id: STR,
    status: { type: 'string', enum: ['ok', 'falhou', 'bloqueado'] },
    arquivos: LISTA,
    divergencias: LISTA,
    observacoes: STR,
  },
}

const POS = {
  type: 'object',
  required: ['ok', 'contagem', 'falhas'],
  properties: {
    ok: { type: 'boolean' },
    contagem: STR,
    falhas: {
      type: 'array',
      items: {
        type: 'object',
        required: ['task_id', 'tipo', 'motivo'],
        properties: {
          task_id: STR,
          tipo: { type: 'string', enum: ['i', 'ii'] },
          motivo: STR,
          brief_correcao: STR,
        },
      },
    },
  },
}

// args pode ser { meta_path, only? } (metadados num arquivo, lidos por um agente barato) ou o objeto completo.
let cfg = args
if (args.meta_path) {
  const TASKM = {
    type: 'object',
    required: ['id', 'tier', 'final', 'arquivos'],
    properties: { id: STR, tier: STR, final: { type: 'boolean' }, arquivos: LISTA },
  }
  const GROUPM = {
    type: 'object',
    required: ['id', 'verificacao_grupo', 'tasks'],
    properties: { id: STR, verificacao_grupo: STR, tasks: { type: 'array', items: TASKM } },
  }
  cfg = await agent(
    `Leia o arquivo JSON ${args.meta_path} e devolva EXATAMENTE o seu conteúdo no schema (não resuma, não omita nenhum grupo nem task).`,
    {
      agentType: 'operario-mecanico',
      label: 'meta',
      schema: {
        type: 'object',
        required: ['stream', 'graph_path', 'repo', 'groups'],
        properties: { stream: STR, graph_path: STR, repo: STR, groups: { type: 'array', items: GROUPM } },
      },
    },
  )
}
const { stream, graph_path, repo, groups } = cfg
const only = args.only

const REGRAS =
  `Projeto cosmic-cosmetics, repositório ${repo}, fluxo ${stream}. REGRAS INVIOLÁVEIS: NÃO faça commit nem qualquer git que grave ` +
  `(add/commit/stash/reset/push); toque SOMENTE os arquivos permitidos da sua task (outros agentes editam o resto ao mesmo tempo); ` +
  `NÃO rode cargo/build/testes (o target é compartilhado por dezenas de agentes; o supervisor roda a verificação do grupo); ` +
  `nunca leia segredos (PIN/keyring); mensagens ao usuário em PT-BR; arquivo <800 linhas, função <50. ` +
  `NUNCA execute binários instaladores/shims reais, sudo, pkexec ou systemctl de verdade, e NUNCA escreva fora do repositório, do HOME de teste ou de diretórios temporários (testes usam Env::fake/tempdir). ` +
  `NUNCA use pkill, killall nem kill por padrão de nome (pkill -f casa com a sessão do usuário e com o seu próprio shell): só mate processos que VOCÊ iniciou, por PID registrado. ` +
  `TDD: escreva os testes pedidos no brief ANTES do código, no mesmo arquivo/par de arquivos. Troque TODOS os todo!()/unimplemented!() ` +
  `dos seus arquivos por implementação real (nenhum todo!() pode sobrar nos seus arquivos). ` +
  `Licença de corrigir o plano: se o código/assinatura do brief contradisser o esqueleto real ou não compilar, ajuste para satisfazer ` +
  `o teste e relate a divergência; nunca altere um teste para acomodar implementação defeituosa; não mude assinaturas PÚBLICAS do ` +
  `esqueleto (outros agentes dependem delas) — se for inevitável, relate como status "bloqueado".`

const briefOperario = (g, t) =>
  `${REGRAS}\n\n--- SUA MICRO-TASK ---\nGrupo: ${g.id}  Task: ${t.id}\n` +
  `Arquivos permitidos: ${t.arquivos.join(', ')}\n` +
  `Leia o arquivo ${graph_path}: use o campo "contexto_fixo" (raiz) e o campo "brief" da task de id "${t.id}" ` +
  `(em grupos[].tasks[]). Execute o brief. Responda com o JSON do schema (status ok só se terminou tudo).`

const tipo = t => (t.tier === 'haiku' ? 'operario-mecanico' : 'operario')
// tier 'opus' → operário com capability bump (grab de teclado, concorrência, segurança)
const opcoesTier = t => (t.tier === 'opus' ? { model: 'opus' } : {})

const resultados = []
let parado = null

for (const g of groups) {
  if (only && !only.includes(g.id)) continue
  const paralelas = g.tasks.filter(t => !t.final)
  const finais = g.tasks.filter(t => t.final)

  const soVerifica = (args.verify_only || []).includes(g.id)
  let relatorios = []
  if (soVerifica) {
    log(`${stream}/${g.id}: só verificação/correção (operários já rodaram antes)`)
  } else {
    phase('Operários')
    log(`${stream}/${g.id}: ${paralelas.length} operários em paralelo + ${finais.length} finais`)
    relatorios = (
      await pipeline(paralelas, t =>
        agent(briefOperario(g, t), { agentType: tipo(t), label: `${stream}:${t.id}`, phase: 'Operários', schema: RELATORIO, ...opcoesTier(t) }),
      )
    ).filter(Boolean)
    for (const t of finais) {
      const r = await agent(briefOperario(g, t), { agentType: tipo(t), label: `${stream}:${t.id}`, phase: 'Operários', schema: RELATORIO, ...opcoesTier(t) })
      if (r) relatorios.push(r)
    }
  }
  const bloqueados = relatorios.filter(r => r.status !== 'ok')
  if (bloqueados.length) log(`${stream}/${g.id}: ${bloqueados.length} relatório(s) não-ok: ${bloqueados.map(b => b.task_id).join(', ')}`)

  let pos = null
  const rodadasLog = []
  let infra = 0
  for (let rodada = 0; rodada <= 2; rodada++) {
    phase('Verificação')
    pos = await agent(
      `Você é o SUPERVISOR do grupo ${g.id} do fluxo ${stream} (projeto cosmic-cosmetics, repo ${repo}). NÃO edite arquivos e NÃO commite. ` +
        `Rode a verificação do grupo no host, exatamente: ${g.verificacao_grupo}\n` +
        `GATE ANTI-STUB (obrigatório, além dos testes): rode no host um grep por \`todo!(\` e \`unimplemented!(\` nos arquivos .rs das tasks deste grupo ` +
        `(lista abaixo; ignore caminhos em /tests/ e blocos #[cfg(test)]). QUALQUER ocorrência é uma falha de tipo "i" da task dona do arquivo ` +
        `(motivo: 'stub restante'; brief_correcao: implementar de verdade conforme o brief da task em ${graph_path}, com testes), e ok só pode ser true se não houver nenhuma. ` +
        `EXCEÇÃO: um todo!("X") cuja marca X cite o id de uma task de OUTRO grupo (ainda não executado) num arquivo COMPARTILHADO NÃO reprova este grupo — registre-o apenas em "motivo" de uma falha tipo "ii" SÓ se não houver outra falha; se tudo passar, ok=true e liste os stubs adiados em "contagem". ` +
        `Se compilar e passar tudo: ok=true e contagem com os números reais (ex.: "42 passed, 0 failed"). ` +
        `Se falhar: classifique CADA falha por task_id usando os arquivos de cada task (tabela abaixo; erro de compilação num arquivo ⇒ a task dona dele) ` +
        `com tipo "i" (bug do operário: corrigível) ou "ii" (defeito do brief/esqueleto/assinatura compartilhada: não corrigível por um operário só), ` +
        `motivo claro (com o trecho do erro) e, para tipo "i", um brief_correcao AUTOSSUFICIENTE (arquivo, erro exato, o que mudar). ` +
        `Se o build travar esperando o lock do cargo, aguarde (outros fluxos compilam ao mesmo tempo). Se a falha vier de arquivo de OUTRO fluxo, ` +
        `espere ~2 min e rode de novo uma vez antes de classificar.\n\nTasks do grupo e seus arquivos:\n` +
        JSON.stringify(g.tasks.map(t => ({ id: t.id, arquivos: t.arquivos })), null, 1) +
        `\n\nRelatórios dos operários:\n${JSON.stringify(relatorios, null, 1)}`,
      { agentType: 'supervisor', label: `${stream}:pos ${g.id} r${rodada}`, phase: 'Verificação', schema: POS },
    )
    if (!pos) {
      infra++
      log(`${stream}/${g.id}: verificação falhou por erro de infraestrutura (tentativa ${infra}); repetindo`)
      if (infra > 3) {
        pos = { ok: false, contagem: 'verificação indisponível (erro de infraestrutura)', falhas: [] }
        break
      }
      rodada--
      continue
    }
    rodadasLog.push({ rodada, ok: pos.ok, contagem: pos.contagem })
    const corrigiveis = pos.falhas.filter(f => f.tipo === 'i' && f.brief_correcao)
    if (pos.ok || corrigiveis.length === 0 || rodada === 2) break

    phase('Correção')
    const opcoesBase = { agentType: 'operario', phase: 'Correção', schema: RELATORIO }
    const corr = (
      await pipeline(corrigiveis, f => {
        const orig = g.tasks.find(x => x.id === f.task_id) || { id: f.task_id, arquivos: [], tier: 'sonnet' }
        const opc = { ...opcoesBase, label: `${stream}:fix ${f.task_id} r${rodada + 1}` }
        if (rodada === 1) opc.model = 'opus'
        return agent(
          `${REGRAS}\n\n--- CORREÇÃO (rodada ${rodada + 1}) ---\nGrupo ${g.id}, task ${f.task_id}. Arquivos permitidos: ` +
            `${orig.arquivos.join(', ')} (se o erro exigir tocar outro arquivo DO MESMO GRUPO, faça o mínimo e relate).\n` +
            `Falha: ${f.motivo}\nInstrução: ${f.brief_correcao}\nO brief original está em ${graph_path} (task "${f.task_id}") se precisar.`,
          opc,
        )
      })
    ).filter(Boolean)
    relatorios.push(...corr)
  }

  const res = {
    grupo: g.id,
    ok: pos.ok,
    contagem: pos.contagem,
    rodadas: rodadasLog,
    falhas_restantes: pos.ok ? [] : pos.falhas,
  }
  resultados.push(res)

  if (pos.ok) {
    await agent(
      `Registre o grupo concluído. Arquivo: ${repo}/.superpowers/sdd/wave2/${stream}/groups-done.json (JSON: lista de ids de grupos; ` +
        `crie com [] se não existir). Acrescente "${g.id}" se ainda não estiver, mantendo o JSON válido. Use só as ferramentas Read/Write/Edit/Bash. ` +
        `Responda apenas "ok".`,
      { agentType: 'operario-mecanico', label: `${stream}:registro ${g.id}`, phase: 'Verificação' },
    )
  } else {
    parado = g.id
    log(`${stream}: PAROU no grupo ${g.id} (verificação não passou após 2 correções) — grupos seguintes dependem dele.`)
    break
  }
}

return { stream, parado, grupos: resultados }
