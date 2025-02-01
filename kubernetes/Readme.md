# Commands

### List

- `kubectl get pods` - List all pods in the current namespace
- `kubectl get pods -n <namespace>` - List all pods in the specified namespace
- `kubectl get pods -o wide` - List all pods in the current namespace with more details
- `kubectl get pods -o wide -n <namespace>` - List all pods in the specified namespace with more
  details
- `kubectl get pods -o wide --show-labels` - List all pods in the current namespace with more
  details and labels
- `kubectl get pods -o wide --show-labels -n <namespace>` - List all pods in the specified namespace
  with more details and labels

#### Volumes

- `kubectl get pv` - List all persistent volumes
- `kubectl get pv -o wide` - List all persistent volumes with more details
- `kubectl get pvc` - List all persistent volume claims
- `kubectl get pvc -o wide` - List all persistent volume claims with more details
- `kubectl get sc` - List all storage classes
- `kubectl get sc -o wide` - List all storage classes with more details
- `kubectl get storageclass` - List all storage classes

### Describe

- `kubectl describe pod <pod-name>` - Describe a pod in the current namespace
- `kubectl describe pod <pod-name> -n <namespace>` - Describe a pod in the specified namespace
- `kubectl describe pod <pod-name> -o yaml` - Describe a pod in the current namespace in yaml format

### Create

- `kubectl create -f <file>` - Create a resource from a file
  namespace

### Apply

- `kubectl apply -f <file>` - Apply a configuration to a resource

### Delete

- `kubectl delete pod <pod-name>` - Delete a pod in the current namespace
- `kubectl delete pod <pod-name> -n <namespace>` - Delete a pod in the specified namespace
- `kubectl delete pod --all` - Delete all pods in the current namespace

## Helm

### Install

- `helm install <release-name> <chart-name>` - Install a Helm chart
- `helm install <release-name> <chart-name> -n <namespace>` - Install a Helm chart in a specific
  namespace
- `helm install <release-name> <chart-name> --values <values-file> --create-namespace --namespace` -
  Install a Helm chart with custom values and create a namespace if it doesn't exist

### Logs

- `kubectl logs <pod-name>` - Get logs from a pod in the current namespace
- `kubectl logs <pod-name> -n <namespace>` - Get logs from a pod in the specified namespace
- `kubectl logs elasticsearch-es-masters-0 -n elastic | grep "ERROR"`

### Service

- `kubectl get svc` - List all services in the current namespace
- `kubectl get svc -n <namespace>` - List all services in the specified namespace
- `kubectl get svc -o wide` - List all services in the current namespace with more details

### Secrets

- ` kubectl get secret scylla-tls-secret -n scylla -o yaml` - Get a secret in yaml format

# MiniKube

- `minikube start --cpus=12 --memory=12000 --driver=kvm2 --disk-size 50GB`
- `minikube ssh`
- set fs.aio-max-nr = 1048576 in /etc/sysctl.conf
- `sudo sysctl -p /etc/sysctl.conf`
- prepare volumes
  ```ssh
  sudo mkdir -p /mnt/data-scylla-0 /mnt/data-scylla-1 /mnt/data-scylla-manager
  sudo chmod 777 /mnt/data-scylla-0 /mnt/data-scylla-1 /mnt/data-scylla-manager
  sudo mkdir -p /mnt/data/elasticsearch-0
  sudo chmod 777 /mnt/data/elasticsearch-0 
  ```
- exit miniKube
- `kubectl label node minikube scylla.scylladb.com/node-type=scylla --overwrite`
- `kubectl label node minikube elastic/node-type=elastic --overwrite`
- cert-manager
  ```shell
  helm install \
    cert-manager jetstack/cert-manager \
    --namespace cert-manager \
    --create-namespace \
    --version v1.16.3 \
    --set crds.enabled=true
  ```
- `kubectl apply -f self-issuer.yaml`
- `minikube addons enable ingress`
- `kubectl apply -f scylla/scylla-local-xfs.yaml` -- hack to create a local xfs storage class
- `kubectl apply -f volumes/` -- create the volumes
- `kubectl creeate namespace scylla-operator`
- `kubectl create namespace scylla-manager`
- `kubectl create namespace scylla`
- `kubectl create namespace elastic`
- `kubectl create namespace valkey`
- `kubectl create namespace nodecosmos`
- `kubectl apply -f certificates/` -- create the tls secret
- install scylla operator
  ```shell
  helm install scylla-operator scylla/scylla-operator --values scylla/scylla-operator.yaml --create-namespace --namespace scylla-operator
  ```
- maybe we need to patche the scylla-operator
    - ```shell
      kubectl get secret scylla-tls-secret -n scylla-operator -o jsonpath='{.data.ca\.crt}' | base64 --decode > my-ca.crt
      ```
    - ```shell
        base64 -w0 my-ca.crt
      ```
    - `kubectl get validatingwebhookconfiguration`
    - ```shell
      kubectl patch validatingwebhookconfiguration scylla-operator \
      --type='json' \
      -p='[{"op": "replace", "path": "/webhooks/0/clientConfig/caBundle", "value": "<base64-encoded-ca>"}]'
      ```
- install scylla manager
  ```shell
  helm install scylla-manager scylla/scylla-manager \
  --values scylla/scylla-manager.yaml \
  --create-namespace \
  --namespace scylla-manager
  ```
- install scylla cluster
  ```shell
    helm install scylla scylla/scylla-cluster \
    --values scylla/scylla-cluster.yaml \
    --create-namespace \
    --namespace scylla
    ```
- install elastic operator
  ```shell
  helm install elastic-operator elastic/eck-operator \
  --create-namespace \
  --namespace elastic
  ```
- install elastic cluster
  ```shell
  helm install elasticsearch elastic/elasticsearch \
  --values elastic/elasticsearch.yaml \
  --create-namespace \
  --namespace elastic
  ```
- `kubectl create namespace valkey`
- install valkey
  ```shell
  helm install valkey valkey/valkey \
  --values valkey/valkey.yaml \
  --create-namespace \
  --namespace valkey
  ```

# Shell

- `kubectl exec -it <pod-name> -- /bin/bash` - Open a shell in a pod in the current namespace
- `kubectl exec -it <pod-name> -n <namespace> -- /bin/bash` - Open a shell in a pod in the specified
  namespace
- `kubectl exec -it <pod-name> -c <container-name> -- /bin/bash` - Open a shell in a specific
  container in a pod in the current namespace