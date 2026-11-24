//! Desktop input and original-attempt settlement on the independent DA device listener.
use super::*;
use crate::{access_worker as access, credential_worker::state};
use access::macos::{self, creation::{self, Evidence, Outcome}};
use execution_contract::Id;
use execution_ipc::device_access::{EmployeePortal, SetupTask};
use rss_mdm_agent_wire::{da::{AccountEffect, AccountObservation as Event, AccountOffer, AccountSettlement},runtime as w};
use std::{collections::BTreeMap, path::PathBuf, sync::Mutex};
use zeroize::Zeroizing;

type Ids = (Uuid,Uuid);
fn ids(t: &w::AccessTaskSpec) -> Ids { (t.task_id,t.attempt_id) }
fn seconds() -> Result<i64,Error> { Ok((crate::wall()? / 1000) as i64) }

#[derive(Clone)]
pub(crate) struct Employee {
    pub portal: EmployeePortal,
    pub principal: Uuid,
    pub proof: Zeroizing<String>,
    pub subject: String,
    pub session: u32,
    pub binding: Id,
    pub epoch: Arc<Mutex<u64>>,
    pub generation: u64,
    pub current: Arc<AtomicBool>,
    pub expires: u64,
}
impl Employee {
    fn permits(&self, task: &w::AccessTaskSpec) -> bool {
        (self.portal.tenant, self.portal.instance, self.principal) == (task.tenant_id,task.instance_id,task.principal_id)
            && self.current.load(Ordering::Acquire)
            && crate::wall().is_ok_and(|now| now < self.expires)
            && self.epoch.lock().is_ok_and(|e| *e == self.generation)
            && execution_ipc::macos_identity::verify_console(&self.subject,self.session,&self.binding).is_ok()
    }
    async fn verify(&self, t: &w::AccessTaskSpec) -> Result<(),Error> {
        if !self.permits(t) { return Err(Error::Identity); }
        let expires = da::verify_employee(&self.portal.origin,self.portal.ca_pem.as_deref(),self.portal.tenant,self.principal,&self.proof).await?;
        if expires <= seconds()? || !self.permits(t) { return Err(Error::Identity); }
        Ok(())
    }
}
struct Input { task: w::AccessTaskSpec, owner: Employee, bytes: Zeroizing<String>, until: std::time::Instant }
#[derive(Default)]
struct State { offers: BTreeMap<Ids,w::AccessTaskSpec>, input: Option<Input> }
#[derive(Default)]
pub(crate) struct Setup { state: Mutex<State>, pub wake: tokio::sync::Notify }
impl Setup {
    pub fn owns(&self, id: Ids) -> Result<bool,Error> { Ok(self.state.lock().map_err(|_|Error::Storage)?.offers.contains_key(&id)) }
    pub fn tasks(&self, e: &Employee) -> Result<Vec<SetupTask>,Error> {
        let now = seconds()?;
        let s = self.state.lock().map_err(|_|Error::Storage)?;
        Ok(s.offers.values().filter(|t| t.expires_at > now && e.permits(t)).filter_map(|t| {
            let w::AccessAction::Create {name,..} = &t.action else { return None; };
            Some(SetupTask {task:t.task_id,attempt:t.attempt_id,binding:t.binding_id,create_name:Some(name.clone())})
        }).collect())
    }
    pub fn prepare(self:&Arc<Self>, id: Ids, e: Employee, password: &Zeroizing<String>) -> Result<(),Error> {
        let mut s = self.state.lock().map_err(|_|Error::Storage)?;
        let t = s.offers.get(&id).ok_or(Error::NotFound)?;
        if !e.permits(t) || t.expires_at <= seconds()? || password.is_empty() || password.len()>1024 || password.chars().any(char::is_control) { return Err(Error::Denied); }
        let until=std::time::Instant::now()+Duration::from_secs(30);
        s.input = Some(Input { task:t.clone(), owner:e, bytes:password.clone(), until });
        let setup=Arc::downgrade(self);
        tokio::spawn(async move {
            tokio::time::sleep_until(until.into()).await;
            if let Some(setup)=setup.upgrade(){
                if let Ok(mut state)=setup.state.lock() {
                    if state.input.as_ref().is_some_and(|i|i.until<=std::time::Instant::now()){state.input=None;}
                }
            }
        });
        self.wake.notify_one();
        Ok(())
    }
    pub fn clear_input(&self) -> Result<(),Error> { self.state.lock().map_err(|_|Error::Storage)?.input=None; Ok(()) }
    pub fn reset(&self) -> Result<(),Error> { *self.state.lock().map_err(|_|Error::Storage)?=State::default(); Ok(()) }
    fn withdraw(&self,id:Ids) -> Result<(),Error> {
        let mut s=self.state.lock().map_err(|_|Error::Storage)?;
        s.offers.remove(&id);
        if s.input.as_ref().is_some_and(|i| ids(&i.task)==id) { s.input=None; }
        Ok(())
    }
    fn offer(&self,t:&w::AccessTaskSpec)->Result<(),Error>{
        let now=seconds()?;
        let mut s=self.state.lock().map_err(|_|Error::Storage)?;
        s.offers.retain(|_,o|o.expires_at>now && (o.task_id!=t.task_id || o.attempt_id==t.attempt_id));
        if s.input.as_ref().is_some_and(|i|i.task.task_id==t.task_id && i.task!=*t){s.input=None;}
        if s.offers.len()>=64 && !s.offers.contains_key(&ids(t)) { return Err(Error::Capacity); }
        s.offers.insert(ids(t),t.clone());
        Ok(())
    }
    fn confirmed(&self) -> Result<Option<(w::AccessTaskSpec,Employee)>,Error> {
        let mut s=self.state.lock().map_err(|_|Error::Storage)?;
        if s.input.as_ref().is_some_and(|i| i.until <= std::time::Instant::now() || !i.owner.permits(&i.task)) { s.input=None; }
        Ok(s.input.as_ref().map(|i|(i.task.clone(),i.owner.clone())))
    }
    fn take(&self,t:&w::AccessTaskSpec,owner:&Employee) -> Result<Zeroizing<String>,Error> {
        let mut s=self.state.lock().map_err(|_|Error::Storage)?;
        let i=s.input.as_ref().ok_or(Error::Denied)?;
        if i.task!=*t || !Arc::ptr_eq(&owner.current,&i.owner.current) || i.until<=std::time::Instant::now() || !i.owner.permits(t) || seconds()?>=t.expires_at {return Err(Error::Denied);}
        Ok(s.input.take().ok_or(Error::Denied)?.bytes)
    }
}

fn validate(
    registration:&rss_mdm_agent_wire::da::Registration, offer:&AccountOffer, ack:&AccountSettlement, t:&w::AccessTaskSpec,
    previous:Option<&w::AccessTaskSpec>, now:i64
) -> Result<(),Error> {
    t.validate()?;
    if (registration.tenant,registration.device)!=(t.tenant_id,t.device_id)
        || (offer.task_id,offer.attempt_id,offer.binding_id,offer.definition_digest)!=(t.task_id,t.attempt_id,t.binding_id,t.definition_digest)
        || (t.platform,t.architecture,t.permit)!=(w::TaskPlatform::Macos,w::TaskArchitecture::Aarch64,w::TaskPermit::Start)
        || t.windows.is_some() || t.expires_at<=now || t.deadline<=now || ack.issued_expires_at!=Some(t.expires_at) {
        return Err(Error::Untrusted);
    }
    if let Some(old)=previous {
        let mut renewed=old.clone();
        renewed.expires_at=t.expires_at;
        renewed.deadline=t.deadline;
        if renewed!=*t {return Err(Error::Conflict);}
    }
    Ok(())
}

fn result(t:&w::AccessTaskSpec,effect:impl Into<AccountEffect>)->rss_mdm_agent_wire::da::AccountResult{
    w::AccessResult {binding_id:t.binding_id,binding_generation:t.binding_generation,revision:t.revision,definition_digest:t.definition_digest,effect:effect.into()}
}

fn created(t:&w::AccessTaskSpec,a:macos::NativeAccount)->Result<rss_mdm_agent_wire::da::AccountResult,Error>{
    let actual=macos::facts(&a);
    let generated_uid=Uuid::parse_str(&a.generated_uid).map_err(|_|Error::Identity)?;
    Ok(result(t,AccountEffect::MacAccountCreated { generated_uid,
        role:if a.administrator {w::AccessRole::Administrator} else {w::AccessRole::Standard},
        secure_token:actual.secure_token,volume_owner:actual.volume_owner,filevault_unlock:actual.filevault_unlock }))
}

async fn blocking<T: Send+'static>(f:impl FnOnce()->Result<T,Error> + Send+'static)->Result<T,Error>{
    tokio::task::spawn_blocking(f).await.map_err(|_|Error::Unavailable)?
}
#[derive(Clone)]
struct Journal { root:PathBuf, native:PathBuf }
impl Journal {
    fn new(d:&Deployment,device:rss_mdm_agent_wire::DeviceId)->Result<Self,Error>{
        let root=d.da_root()?.join(format!("account-{device}"));
        let native=access::root(&d.execution);
        platform_private_storage::directory(&root)?;
        platform_private_storage::directory(&d.execution.material_root)?;
        platform_private_storage::directory(&native)?;
        Ok(Self{ root,native })
    }
    fn saved(&self,t:&w::AccessTaskSpec)->Result<Option<rss_mdm_agent_wire::da::AccountResult>,Error>{
        let value:Option<rss_mdm_agent_wire::da::AccountResult>=state::read_optional(&self.root,&access::name(t,"result"))?;
        if value.as_ref().is_some_and(|v|(v.binding_id,v.binding_generation,v.revision,v.definition_digest)!=(t.binding_id,t.binding_generation,t.revision,t.definition_digest)){return Err(Error::Identity);}
        Ok(value)
    }
    fn save(&self,t:&w::AccessTaskSpec,value:&rss_mdm_agent_wire::da::AccountResult)->Result<(),Error>{
        state::write(&self.root,&access::name(t,"result"),value)
    }
    fn evidence(&self,t:&w::AccessTaskSpec)->Result<Evidence,Error>{
        let _lock=state::lock(&self.native)?;
        match &t.action {
            w::AccessAction::Create{name,..}=>creation::evidence(&self.root,t,name),
            _=> Ok(Evidence::Unstarted),
        }
    }
    fn cleanup(&self,id:Uuid,attempt_id:Uuid,done:da::AccountCompletion)->Result<(),Error>{
        let _lock=state::lock(&self.native)?;
        attempt::retire(&self.root,id,attempt_id,done,|t|{
            state::remove(&self.root,&access::name(t,"creation"))?;
            state::remove(&self.root,&access::name(t,"result"))
        })
    }
}

fn keep(root:&Path,service:Uuid,t:&w::AccessTaskSpec,mutate:bool)->Result<w::AccessEffect,Error>{
    use crate::service::platform_sso::{binding_name,save_binding};
    let w::AccessAction::Unbind { account:w::AccessAccount::Macos{generated_uid}, disposition:w::AccessAccountDisposition::Keep }=&t.action else {
        return Ok(w::AccessEffect::Failed{reason:w::AccessFailure::Unsupported});
    };
    let _lock=state::lock(root)?;
    let key=binding_name(&generated_uid.to_string())?;
    if let Some(mut b)=state::read_optional::<platform_sso::LocalBinding>(root,&key)? {
        if b.authority!=rss_mdm_agent_wire::platform_sso::RegistrationTrust::DaDevicePop ||
            (b.installation,b.tenant,b.device_id,b.binding)!=(service,t.tenant_id,t.device_id,t.binding_id) ||
            b.binding_generation>t.binding_generation || b.revision>t.revision {
            return Ok(w::AccessEffect::Failed{reason:w::AccessFailure::AccountChanged});
        }
        if b.enabled {
            if !mutate || t.expires_at<=seconds()? {return Ok(w::AccessEffect::Unknown);}
            b.enabled=false;
            b.binding_generation=t.binding_generation;
            b.revision=t.revision;
            save_binding(root,&b)?;
        }
    }
    Ok(w::AccessEffect::Unbound{disposition:w::AccessAccountDisposition::Keep})
}

enum Step { Effect(rss_mdm_agent_wire::da::AccountResult), Created(macos::NativeAccount) }

pub(crate) async fn drive(d:&Deployment,client:&da::Client<DeviceSecrets,SystemClock>,setup:&Arc<Setup>)->Result<(),Error>{
    if !cfg!(target_arch="aarch64") {return Ok(());}
    let ops=client.operations()?;
    let registration=ops.registration();
    let journal=Journal::new(d,registration.device)?;
    ops.replay_account().await?;
    ops.retire_account(|task,attempt,done|{
        journal.cleanup(task,attempt,done)?;
        setup.withdraw((task,attempt))
    })?;
    let input=setup.confirmed()?;
    let Some(offer)=ops.account(w::TaskPlatform::Macos,input.as_ref().map(|(t,_)|ids(t))).await? else {return Ok(());};
    let id=(offer.task_id,offer.attempt_id);
    let key=attempt::key(id.0,id.1);
    let mut record:attempt::Attempt=state::read_optional(&journal.root,&key)?.unwrap_or_default();
    let request=record.renew_from.map_or(Event::Received,|expires_at|Event::NotStarted{expires_at});
    let ack=match ops.account_event(&offer,request.clone()).await {
        Ok(ack)=>ack, Err(e)=>{setup.withdraw(id)?;return Err(e);}
    };
    let permit=ack.permit.as_ref();
    if let Some(t)=permit {
        validate(registration,&offer,&ack,t,record.permit.as_ref(),seconds()?)?;
        record.permit=Some(t.clone());
        state::write(&journal.root,&key,&record)?;
    } else { setup.withdraw(id)?; }
    let retained=record.permit.clone();
    let task=retained.as_ref();
    if let Some(t)=task {
        if let Some(r)=journal.saved(t)? {
            ops.account_event(&offer,Event::Result(r)).await?;
            return Ok(());
        }
    }
    let evidence=match task {
        Some(t)=>{let j=journal.clone();let t=t.clone();blocking(move||j.evidence(&t)).await?}
        None=>Evidence::Unstarted,
    };
    let expires=ack.issued_expires_at.filter(|v|seconds().is_ok_and(|now|*v<=now));
    if matches!(evidence,Evidence::Unstarted) && permit.is_none() && !record.started {
        if let Some(expires_at)=expires {
            record.renew_from=Some(expires_at);
            record.started=false;
            state::write(&journal.root,&key,&record)?;
            ops.account_event(&offer,Event::NotStarted{expires_at}).await?;
        }
        return Ok(());
    }
    let Some(t)=task else {return Err(Error::Protocol);};
    let step=match evidence {
        Evidence::Created(a)=>Step::Created(a),
        Evidence::Absent if expires.is_some()=>{
            ops.account_event(&offer,Event::CreateAbsent{expires_at:expires.ok_or(Error::Protocol)?}).await?;
            return Ok(());
        }
        Evidence::Absent | Evidence::Unknown=>Step::Effect(result(t,w::AccessEffect::Unknown)),
        Evidence::Unstarted=>{
            if let w::AccessAction::Create{name,configuration}=&t.action {
                setup.offer(t)?;
                let Some((_,employee))=input.filter(|(confirmed,_)|confirmed==t) else {return Ok(());};
                employee.verify(t).await?;
                if ops.account_event(&offer,request.clone()).await?.permit.as_ref()!=Some(t) {setup.withdraw(id)?;return Ok(());}
                let actual=Deployment::metadata(&Deployment::default_path()?)?;
                actual.require_service()?;
                if unsafe{libc::geteuid()}!=0 || actual.installation_id!=d.installation_id || actual.da_root()?!=d.da_root()? || access::root(&actual.execution)!=journal.native || actual.modules.da.backend()?!=d.modules.da.backend()? {return Err(Error::Identity);}
                let j=journal.clone();let spec=t.clone();let account_name=name.clone();
                let setup=setup.clone();let actor=employee.clone();
                let new=blocking(move|| {
                    let _lock=state::lock(&j.native)?;
                    if !actor.permits(&spec) || spec.expires_at<=seconds()? {return Err(Error::Denied);}
                    match creation::create(&j.root,&spec,&account_name,||setup.take(&spec,&actor)){
                        Ok(Outcome::Created(a))=>Ok(Step::Created(a)),
                        Ok(Outcome::NameConflict)=>{
                            let r=result(&spec,w::AccessEffect::Failed{reason:w::AccessFailure::NameConflict});
                            j.save(&spec,&r)?; Ok(Step::Effect(r))
                        }
                        Err(e) if matches!(creation::evidence(&j.root,&spec,&account_name)?,Evidence::Unstarted)=>Err(e),
                        Err(_)=>Ok(Step::Effect(result(&spec,w::AccessEffect::Unknown))),
                    }
                }).await?;
                if let Step::Created(ref a)=new {
                    if let Some(role)=configuration.role.filter(|r|(*r==w::AccessRole::Administrator)!=a.administrator) {
                        if employee.permits(t) && ops.account_event(&offer,request).await?.permit.as_ref()==Some(t) {
                            let j=journal.clone();let uid=a.generated_uid.clone();let spec=t.clone();
                            blocking(move||{
                                let _lock=state::lock(&j.native)?;
                                if employee.permits(&spec) && seconds()?<spec.expires_at {
                                    let known=macos::inspect(&uid,false)?;
                                    let _=macos::change_role(&known,Some(role));
                                }
                                Ok(())
                            }).await?;
                        }
                    }
                }
                new
            } else {
                // Only the explicit disposition permit can change this DA-owned binding.
                let may_start = !record.started && permit.is_some();
                let spec=t.clone();
                let native=journal.native.clone();
                let service=registration.service_id;
                if may_start { record.started=true; state::write(&journal.root,&key,&record)?; }
                Step::Effect(result(t,blocking(move||keep(&native,service,&spec,may_start)).await?))
            }
        }
    };
    setup.withdraw(id)?;
    let r=match step {
        Step::Effect(r)=>r,
        Step::Created(a)=>{
            let spec=t.clone();blocking(move||match macos::inspect(&a.generated_uid,false){
                Ok(current)=>created(&spec,current),
                Err(_)=>Ok(result(&spec,w::AccessEffect::Unknown)),
            }).await?
        }
    };
    if !matches!(r.effect,AccountEffect::Access(w::AccessEffect::Unknown)){journal.save(t,&r)?;}
    ops.account_event(&offer,Event::Result(r)).await?;
    Ok(())
}
